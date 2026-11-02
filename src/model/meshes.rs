use super::Model;
use super::args::{Args, label_of, point3, positive};
use crate::mesh::sdf::{self, Combine, Primitive, Shape};
use crate::mesh::{self, Mesh, Region, Selection};
use crate::parse::Line;
use anyhow::{Context, Result, anyhow, bail};
use std::sync::Arc;

pub const MESH_OPERATIONS: &[&str] = &["blob", "limb", "remesh", "select", "inflate", "smooth"];

type Named = (String, Vec<Region>, f64, Option<usize>);

#[derive(Clone, Default)]
pub struct MeshBody {
    pub blockout: Vec<Primitive>,
    pub cell: Option<f64>,
    pub mesh: Option<Arc<Mesh>>,
    pub selection: Option<Selection>,
    pub selections: Vec<Named>,
    preview: std::sync::OnceLock<std::result::Result<Arc<Mesh>, String>>,
}

impl MeshBody {
    pub fn is_empty(&self) -> bool {
        self.blockout.is_empty() && self.mesh.is_none()
    }
}

const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn mesh_combine(args: &Args<'_>) -> Result<Combine> {
    match args.values.get("mode").copied() {
        None | Some("add") => Ok(Combine::Add),
        Some("cut") => Ok(Combine::Cut),
        Some("intersect") => Ok(Combine::Intersect),
        Some(other) => bail!("mode must be add, cut or intersect, got `{other}`"),
    }
}

fn axis_index(text: &str) -> Result<usize> {
    match text.to_ascii_lowercase().as_str() {
        "x" => Ok(0),
        "y" => Ok(1),
        "z" => Ok(2),
        other => bail!("mirror takes x, y or z, got `{other}`"),
    }
}

fn numbers(text: &str, scope: &crate::parse::Scope) -> Result<Vec<f64>> {
    crate::parse::split_top_level(text, ',')
        .iter()
        .map(|part| crate::parse::eval(part, scope))
        .collect()
}

impl Model {
    pub fn has_mesh(&self) -> bool {
        !self.mesh_body.is_empty()
    }

    pub(crate) fn mesh_op(&self, op: &str) -> Result<()> {
        if self.has_mesh() {
            bail!(
                "`{op}` works on solids and body `{}` is a mesh; start a solid with `body name`",
                self.body_name()
            );
        }
        Ok(())
    }

    fn start_mesh(&self, op: &str) -> Result<()> {
        if self.solid.is_some() {
            bail!(
                "`{op}` makes a mesh and body `{}` is a solid; start a mesh with `body name`",
                self.body_name()
            );
        }
        self.require_empty_sketch(op)
    }

    pub fn meshes(&self) -> Vec<(String, Arc<Mesh>)> {
        let mut out = self.mesh_bodies.clone();
        if let Some(mesh) = self.current_mesh().ok().flatten() {
            out.push((self.body_name(), mesh));
        }
        out
    }

    pub fn coloured_meshes(&self) -> Vec<(String, Arc<Mesh>, Option<[f64; 3]>)> {
        self.meshes()
            .into_iter()
            .map(|(name, mesh)| {
                let colour = self.colours.get(&name).copied();
                (name, mesh, colour)
            })
            .collect()
    }

    pub(crate) fn current_mesh(&self) -> Result<Option<Arc<Mesh>>> {
        if let Some(mesh) = &self.mesh_body.mesh {
            return Ok(Some(mesh.clone()));
        }
        if self.mesh_body.blockout.is_empty() {
            return Ok(None);
        }
        self.mesh_body
            .preview
            .get_or_init(|| {
                self.blockout_mesh()
                    .map(Arc::new)
                    .map_err(|e| format!("{e:#}"))
            })
            .clone()
            .map(Some)
            .map_err(|e| anyhow!(e))
    }

    fn blockout_mesh(&self) -> Result<Mesh> {
        let cell = self.mesh_body.cell.unwrap_or_else(|| self.default_cell());
        sdf::remesh(&self.mesh_body.blockout, cell)
    }

    fn default_cell(&self) -> f64 {
        let (lo, hi) = self
            .mesh_body
            .blockout
            .iter()
            .map(|p| p.shape.bounds())
            .fold(
                ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]),
                |(a, b), (lo, hi)| {
                    (
                        std::array::from_fn(|k| a[k].min(lo[k])),
                        std::array::from_fn(|k| b[k].max(hi[k])),
                    )
                },
            );
        let size = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
        (size / 120.0).max(1.0e-6)
    }

    pub(crate) fn finish_mesh_body(&mut self) -> Result<()> {
        if let Some(mesh) = self.current_mesh()? {
            self.mesh_bodies.push((self.body_name(), mesh));
        }
        self.mesh_body = MeshBody::default();
        Ok(())
    }

    fn describe_mesh(&self) -> Result<String> {
        let mesh = self
            .current_mesh()?
            .ok_or_else(|| anyhow!("there is no mesh"))?;
        Ok(mesh.summary())
    }

    fn add_primitive(&mut self, line: &Line, shape: Shape, args: &Args<'_>) -> Result<String> {
        if self.mesh_body.mesh.is_some() {
            bail!(
                "`{}` adds to the blockout, which `remesh` has already turned into a mesh; sculpt it with `select` and the mesh ops",
                line.op
            );
        }
        let blend = args.optional_number("blend", &self.scope)?.unwrap_or(0.0);
        if blend < 0.0 {
            bail!("blend must be 0 or more, got {blend}");
        }
        let combine = mesh_combine(args)?;
        let part = label_of(line);
        self.mesh_body.preview = Default::default();
        self.mesh_body.blockout.push(Primitive {
            shape,
            blend,
            combine,
            part,
        });
        Ok(format!(
            "{} primitive(s) in the blockout",
            self.mesh_body.blockout.len()
        ))
    }

    fn rotation_of(&self, args: &Args<'_>) -> Result<[[f64; 3]; 3]> {
        let mut m = IDENTITY;
        for (name, axis) in [
            ("rx", [1.0, 0.0, 0.0]),
            ("ry", [0.0, 1.0, 0.0]),
            ("rz", [0.0, 0.0, 1.0]),
        ] {
            if let Some(angle) = args.optional_number(name, &self.scope)? {
                let r = mesh::rotation(axis, angle);
                m = std::array::from_fn(|i| {
                    std::array::from_fn(|j| (0..3).map(|k| r[i][k] * m[k][j]).sum())
                });
            }
        }
        Ok(m)
    }

    pub(crate) fn op_blob(&mut self, line: &Line) -> Result<String> {
        self.start_mesh("blob")?;
        let args = Args::new(
            line,
            &["at", "r"],
            &["blend", "mode", "rx", "ry", "rz"],
            false,
        )?;
        let centre = point3(args.text("at")?, &self.scope)?;
        let radii = numbers(args.text("r")?, &self.scope)?;
        let radii = match radii[..] {
            [r] => [r; 3],
            [x, y, z] => [x, y, z],
            _ => bail!("`r` is one radius or x,y,z radii"),
        };
        radii
            .iter()
            .try_for_each(|r| positive(*r, "radius").map(|_| ()))?;
        let shape = Shape::Ellipsoid {
            centre: [centre.x, centre.y, centre.z],
            radii,
            rotation: self.rotation_of(&args)?,
        };
        self.add_primitive(line, shape, &args)
    }

    pub(crate) fn op_limb(&mut self, line: &Line) -> Result<String> {
        self.start_mesh("limb")?;
        let args = Args::new(line, &[], &["r", "blend", "mode"], true)?;
        let joints: Vec<mesh::P3> = args
            .rest
            .iter()
            .map(|text| point3(text, &self.scope).map(|p| [p.x, p.y, p.z]))
            .collect::<Result<_>>()?;
        if joints.len() < 2 {
            bail!("`limb` needs at least two joints, like `limb 0,0,0 0,0,10 r=2`");
        }
        let radii = numbers(args.text("r")?, &self.scope)?;
        let radii = match radii.len() {
            1 => vec![radii[0]; joints.len()],
            n if n == joints.len() => radii,
            n => bail!(
                "`r` is one radius or one per joint; there are {} joints and {n} radii",
                joints.len()
            ),
        };
        radii
            .iter()
            .try_for_each(|r| positive(*r, "radius").map(|_| ()))?;
        self.add_primitive(line, Shape::Limb { joints, radii }, &args)
    }

    pub(crate) fn op_remesh(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["cell"], &[], false)?;
        if self.mesh_body.blockout.is_empty() {
            bail!(
                "`remesh` turns a blockout of `blob` and `limb` lines into a mesh; there is none"
            );
        }
        if self.mesh_body.mesh.is_some() {
            bail!("the blockout is already a mesh");
        }
        let cell = match args.optional_number("cell", &self.scope)? {
            Some(cell) => positive(cell, "cell")?,
            None => self.default_cell(),
        };
        self.mesh_body.cell = Some(cell);
        let mesh = self.blockout_mesh()?;
        self.mesh_body.mesh = Some(Arc::new(mesh));
        self.describe_mesh()
    }

    fn sculpted(&mut self, op: &str) -> Result<&mut Mesh> {
        if self.mesh_body.mesh.is_none() {
            if self.mesh_body.blockout.is_empty() {
                bail!(
                    "`{op}` works on a mesh; make one with `blob`/`limb` and `remesh`, or `import file.obj mesh`"
                );
            }
            let mesh = self.blockout_mesh()?;
            self.mesh_body.mesh = Some(Arc::new(mesh));
        }
        let mesh = self.mesh_body.mesh.as_mut().expect("set above");
        Ok(Arc::make_mut(mesh))
    }

    fn region_of(&self, word: &str) -> Result<Vec<Region>> {
        if word == "all" {
            return Ok(vec![Region::All]);
        }
        if let Some((_, regions, _, _)) = self
            .mesh_body
            .selections
            .iter()
            .rev()
            .find(|(name, ..)| name == word)
        {
            return Ok(regions.clone());
        }
        Ok(vec![Region::Part(word.to_string())])
    }

    pub(crate) fn op_select(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(
            line,
            &[],
            &["sphere", "r", "box", "falloff", "mirror"],
            true,
        )?;
        let mut regions = Vec::new();
        for word in &args.rest {
            for name in crate::parse::split_top_level(word, ',') {
                regions.extend(self.region_of(name.trim())?);
            }
        }
        if let Some(text) = args.values.get("sphere") {
            let c = point3(text, &self.scope)?;
            let r = positive(args.number("r", &self.scope)?, "r")?;
            regions.push(Region::Sphere([c.x, c.y, c.z], r));
        } else if args.has("r") {
            bail!("`r=` goes with `sphere=x,y,z`");
        }
        if let Some(text) = args.values.get("box") {
            let v = numbers(text, &self.scope)?;
            let [a, b, c, d, e, f] = v[..] else {
                bail!("`box` is x0,y0,z0,x1,y1,z1");
            };
            regions.push(Region::Box(
                [a.min(d), b.min(e), c.min(f)],
                [a.max(d), b.max(e), c.max(f)],
            ));
        }
        if regions.is_empty() {
            bail!(
                "say what to select: a part name, `all`, `sphere=x,y,z r=`, or `box=x0,y0,z0,x1,y1,z1`"
            );
        }
        let falloff = args.optional_number("falloff", &self.scope)?.unwrap_or(0.0);
        if falloff < 0.0 {
            bail!("falloff must be 0 or more, got {falloff}");
        }
        let mirror = args
            .values
            .get("mirror")
            .map(|t| axis_index(t))
            .transpose()?;
        let mesh = self.sculpted("select")?.clone();
        let selection = Selection::new(&mesh, regions.clone(), falloff, mirror)?;
        let count = selection.count();
        let full = selection.weights.iter().filter(|&&w| w >= 1.0).count();
        let centre = selection.centre;
        self.mesh_body
            .selections
            .push((label_of(line), regions, falloff, mirror));
        self.mesh_body.selection = Some(selection);
        Ok(format!(
            "{count} of {} vertices, {full} at full strength, centre {:.3},{:.3},{:.3}",
            mesh.positions.len(),
            centre[0],
            centre[1],
            centre[2]
        ))
    }

    fn selection_for(&mut self, op: &str) -> Result<Selection> {
        let mesh = self.sculpted(op)?.clone();
        let selection = match self.mesh_body.selection.take() {
            Some(s) => Selection::new(&mesh, s.regions, s.falloff, s.mirror)?,
            None => Selection::new(&mesh, vec![Region::All], 0.0, None)?,
        };
        self.mesh_body.selection = Some(selection.clone());
        Ok(selection)
    }

    fn sculpt_summary(&self, moved: usize, before: &Mesh) -> Result<String> {
        let after = self.mesh_body.mesh.as_ref().expect("sculpted");
        let most = before
            .positions
            .iter()
            .zip(&after.positions)
            .map(|(a, b)| mesh::length(mesh::sub(*a, *b)))
            .fold(0.0, f64::max);
        Ok(format!(
            "moved {moved} vertices, the most by {most:.3}; volume {:.3}",
            after.volume()
        ))
    }

    pub(crate) fn mesh_move(&mut self, by: mesh::P3) -> Result<String> {
        let selection = self.selection_for("move")?;
        let before = self.sculpted("move")?.clone();
        let moved = self
            .sculpted("move")?
            .deform(&selection, |p| mesh::add(p, by));
        self.sculpt_summary(moved, &before)
    }

    pub(crate) fn mesh_rotate(
        &mut self,
        axis: mesh::P3,
        angle: f64,
        about: Option<mesh::P3>,
    ) -> Result<String> {
        let selection = self.selection_for("rotate")?;
        let centre = about.unwrap_or(selection.centre);
        let m = mesh::rotation(axis, angle);
        let before = self.sculpted("rotate")?.clone();
        let moved = self.sculpted("rotate")?.deform(&selection, |p| {
            mesh::add(centre, mesh::apply(&m, mesh::sub(p, centre)))
        });
        self.sculpt_summary(moved, &before)
    }

    pub(crate) fn mesh_scale(
        &mut self,
        factors: mesh::P3,
        about: Option<mesh::P3>,
    ) -> Result<String> {
        let selection = self.selection_for("scale")?;
        let centre = about.unwrap_or(selection.centre);
        let before = self.sculpted("scale")?.clone();
        let flips = factors.iter().filter(|f| **f < 0.0).count() % 2 == 1;
        if flips {
            bail!("scale factors must be positive");
        }
        let moved = self.sculpted("scale")?.deform(&selection, |p| {
            let d = mesh::sub(p, centre);
            mesh::add(
                centre,
                [d[0] * factors[0], d[1] * factors[1], d[2] * factors[2]],
            )
        });
        self.sculpt_summary(moved, &before)
    }

    pub(crate) fn op_inflate(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["by"], &[], false)?;
        let by = args.number("by", &self.scope)?;
        let selection = self.selection_for("inflate")?;
        let before = self.sculpted("inflate")?.clone();
        let moved = self.sculpted("inflate")?.inflate(&selection, by);
        self.sculpt_summary(moved, &before)
    }

    pub(crate) fn op_smooth(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["passes"], &[], false)?;
        let passes = args.optional_number("passes", &self.scope)?.unwrap_or(1.0);
        if passes < 1.0 || passes.fract() != 0.0 || passes > 200.0 {
            bail!("`smooth` takes a whole number of passes from 1 to 200, got {passes}");
        }
        let selection = self.selection_for("smooth")?;
        let before = self.sculpted("smooth")?.clone();
        let moved = self.sculpted("smooth")?.smooth(&selection, passes as usize);
        self.sculpt_summary(moved, &before)
    }

    pub(crate) fn mesh_mirror(&mut self, axis: usize, at: f64, of: Option<&str>) -> Result<String> {
        if self.mesh_body.mesh.is_none() {
            let picked: Vec<Primitive> = self
                .mesh_body
                .blockout
                .iter()
                .filter(|p| of.is_none_or(|name| p.part == name))
                .cloned()
                .collect();
            if picked.is_empty() {
                bail!("no blockout line is labelled `{}`", of.unwrap_or_default());
            }
            self.mesh_body.preview = Default::default();
            for p in picked {
                self.mesh_body.blockout.push(Primitive {
                    shape: p.shape.mirrored(axis, at),
                    part: mirrored_name(&p.part, axis).unwrap_or_else(|| p.part.clone()),
                    ..p
                });
            }
            return Ok(format!(
                "{} primitive(s) in the blockout",
                self.mesh_body.blockout.len()
            ));
        }
        if of.is_some() {
            bail!("`mirror of=` copies blockout lines; after `remesh` mirror the whole mesh");
        }
        let mesh = self.sculpted("mirror")?;
        let mut copy = mesh.transformed(
            |p| {
                let mut q = p;
                q[axis] = 2.0 * at - q[axis];
                q
            },
            true,
        );
        copy.parts = copy
            .parts
            .iter()
            .map(|p| mirrored_name(p, axis).unwrap_or_else(|| p.clone()))
            .collect();
        mesh.merge(&copy);
        self.describe_mesh()
    }

    pub(crate) fn import_mesh(&mut self, line: &Line, path: &std::path::Path) -> Result<String> {
        self.start_mesh("import")?;
        if self.has_mesh() {
            bail!(
                "body `{}` is already a mesh; start another with `body name`",
                self.body_name()
            );
        }
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let mut mesh = match ext.as_str() {
            "obj" => {
                let text = crate::host::read_to_string(path)
                    .with_context(|| format!("reading {}", path.display()))?;
                mesh::read_obj(&text).with_context(|| format!("reading {}", path.display()))?
            }
            "stl" => {
                let bytes = crate::host::read(path)
                    .with_context(|| format!("reading {}", path.display()))?;
                let stl = monstertruck::mesh::stl::read(
                    std::io::Cursor::new(bytes),
                    monstertruck::mesh::stl::StlType::Automatic,
                )
                .map_err(|e| anyhow!("{}: {e}", path.display()))?;
                let positions = stl.positions();
                mesh::from_triangles(
                    stl.faces()
                        .triangle_iter()
                        .map(|t| {
                            t.map(|v| {
                                let p = positions[v.pos];
                                [p.x, p.y, p.z]
                            })
                        })
                        .collect(),
                )
            }
            other => bail!("`import ... mesh` reads .obj and .stl files, not `.{other}`"),
        };
        if mesh.parts == ["mesh"] {
            mesh.parts = vec![label_of(line)];
        }
        self.mesh_body.mesh = Some(Arc::new(mesh));
        self.describe_mesh()
    }
}

fn mirrored_name(name: &str, axis: usize) -> Option<String> {
    const SIDES: [(&str, &str); 4] = [
        ("_l", "_r"),
        ("_r", "_l"),
        ("_left", "_right"),
        ("_right", "_left"),
    ];
    (axis == 0)
        .then(|| {
            SIDES
                .iter()
                .find_map(|(from, to)| name.strip_suffix(from).map(|stem| format!("{stem}{to}")))
        })
        .flatten()
}

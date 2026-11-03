use super::args::{Args, axis, label_of, point3, positive};
use super::solids::{loops, oriented, prism};
use super::{Combine, Model};
use crate::geometry::{self, Frame, Profile};
use crate::parse::{Line, Scope};
use crate::select::{self, GroupEntry};
use anyhow::{Context, Result, anyhow, bail};
use monstertruck::modeling::*;

fn about(point: Point3, transform: Matrix4) -> Matrix4 {
    Matrix4::from_translation(point.to_vec())
        * transform
        * Matrix4::from_translation(-point.to_vec())
}

fn reflection(frame: &Frame) -> Matrix4 {
    let n = frame.normal;
    let linear = Matrix3::from_cols(
        Vector3::unit_x() - n * (2.0 * n.x),
        Vector3::unit_y() - n * (2.0 * n.y),
        Vector3::unit_z() - n * (2.0 * n.z),
    );
    about(frame.origin, Matrix4::from(linear))
}

fn moved(solid: &Solid, transform: Matrix4) -> Solid {
    super::solids::placed(solid, transform)
}

pub(crate) fn moved_surface(surface: &Surface, transform: Matrix4) -> Surface {
    let mut moved = surface.transformed(transform);
    if transform.determinant() < 0.0 {
        moved.invert();
    }
    moved
}

fn copy_flag(args: &Args<'_>) -> Result<bool> {
    match args.values.get("copy").copied() {
        None => Ok(false),
        Some("copy") => Ok(true),
        Some(other) => bail!("`{other}` is not an option; did you mean `copy`?"),
    }
}

impl Model {
    pub(crate) fn body_name(&self) -> String {
        if self.body.is_empty() {
            "main".to_string()
        } else {
            self.body.clone()
        }
    }

    pub fn named_body(&self, name: &str) -> Result<Solid> {
        if name == self.body_name() && self.solid.is_some() {
            return self
                .solid
                .clone()
                .ok_or_else(|| anyhow!("body `{name}` has no solid yet"));
        }
        self.bodies
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s.clone())
            .ok_or_else(|| {
                anyhow!(
                    "no body called `{name}`; bodies are {:?}",
                    self.bodies
                        .iter()
                        .map(|(n, _)| n.clone())
                        .chain(std::iter::once(self.body_name()))
                        .collect::<Vec<_>>()
                )
            })
    }

    pub(crate) fn op_combine(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["into", "from"], &["mode"], false)?;
        let combine = super::args::combine_mode(&args)?;
        let (into, from) = (
            args.text("into")?.to_string(),
            args.text("from")?.to_string(),
        );
        if into == from {
            bail!("combine two different bodies");
        }
        let (a, b) = (self.named_body(&into)?, self.named_body(&from)?);
        let result = match combine {
            Combine::Add => monstertruck::solid::or_normalized(&a, &b),
            Combine::Remove => monstertruck::solid::difference_normalized(&a, &b),
            Combine::Common => monstertruck::solid::and_normalized(&a, &b),
        }
        .map_err(|error| anyhow!("combine failed: {error}"))?;
        let active = self.body_name();
        let mut others: Vec<(String, Solid)> = self
            .bodies
            .drain(..)
            .filter(|(n, _)| *n != into && *n != from)
            .collect();
        if active != into
            && active != from
            && let Some(solid) = self.solid.take()
        {
            others.push((active, solid));
        }
        self.bodies = others;
        self.solid = Some(result);
        self.body = into;
        self.describe_solid()
    }

    pub(crate) fn op_place(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["body"], &["on"], false)?;
        let name = args.text("body")?.to_string();
        let selector = args.text("on")?;
        let tolerance = self.tolerance();
        let target = self
            .bodies
            .iter()
            .map(|(n, s)| (n.clone(), s.clone()))
            .chain(self.solid.clone().map(|s| (self.body_name(), s)))
            .filter(|(n, _)| *n != name)
            .find_map(|(_, solid)| {
                let faces = select::faces(&solid);
                select::select_faces(selector, &solid, &self.groups, tolerance)
                    .ok()?
                    .into_iter()
                    .find_map(|i| match faces[i].oriented_surface() {
                        Surface::Plane(plane) => Some(plane),
                        _ => None,
                    })
            })
            .ok_or_else(|| anyhow!("`{selector}` matched no flat face on the other bodies"))?;
        let normal = target.normal();
        let level = normal.dot(target.origin().to_vec());
        let body = self.named_body(&name)?;
        let lowest = body
            .boundaries()
            .iter()
            .flat_map(|shell| shell.vertex_iter())
            .map(|v| normal.dot(v.point().to_vec()))
            .fold(f64::INFINITY, f64::min);
        let shifted = moved(&body, Matrix4::from_translation(normal * (level - lowest)));
        if name == self.body_name() {
            self.solid = Some(shifted);
        } else if let Some(entry) = self.bodies.iter_mut().find(|(n, _)| *n == name) {
            entry.1 = shifted;
        }
        Ok(format!(
            "moved `{name}` {:.3} onto the face",
            level - lowest
        ))
    }

    fn transform_all(&mut self, transform: Matrix4) -> Result<()> {
        let solid = self
            .solid
            .take()
            .ok_or_else(|| anyhow!("there is no solid to move"))?;
        self.solid = Some(moved(&solid, transform));
        let body = self.body_name();
        self.groups
            .0
            .iter_mut()
            .filter(|entry| entry.body == body)
            .for_each(|entry| entry.surface = moved_surface(&entry.surface, transform));
        self.tools
            .values_mut()
            .flatten()
            .for_each(|(_, tool)| *tool = moved(tool, transform));
        Ok(())
    }

    fn copy_groups(&mut self, label: Option<&str>, transform: Matrix4, as_label: &str) {
        let body = self.body_name();
        let copies: Vec<GroupEntry> = self
            .groups
            .0
            .iter()
            .filter(|entry| entry.body == body)
            .filter(|entry| label.is_none_or(|l| entry.label == l))
            .map(|entry| GroupEntry {
                label: as_label.to_string(),
                group: entry.group.clone(),
                surface: moved_surface(&entry.surface, transform),
                body: entry.body.clone(),
            })
            .collect();
        self.groups.0.extend(copies);
    }

    pub(crate) fn op_mirror(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["on"], &["offset", "of"], false)?;
        if self.has_mesh() {
            let on = args.text("on")?;
            let axis = match on.to_ascii_lowercase().as_str() {
                "x" | "yz" => 0,
                "y" | "xz" => 1,
                "z" | "xy" => 2,
                other => {
                    bail!("a mesh mirrors across x, y or z (the YZ, XZ or XY plane), got `{other}`")
                }
            };
            let at = args.optional_number("offset", &self.scope)?.unwrap_or(0.0);
            return self.mesh_mirror(axis, at, args.values.get("of").copied());
        }
        if let Some(of) = args.values.get("of") {
            let frame = self
                .plane_from(args.text("on")?)?
                .offset(args.optional_number("offset", &self.scope)?.unwrap_or(0.0));
            let transform = reflection(&frame);
            let tools = self
                .tools
                .get(*of)
                .cloned()
                .ok_or_else(|| anyhow!("`{of}` made no material to mirror"))?;
            let label = label_of(line);
            self.copy_groups(Some(of), transform, of);
            for (combine, tool) in tools {
                self.merge(&label, moved(&tool, transform), combine)?;
            }
            return self.describe_solid();
        }
        let frame = self
            .plane_from(args.text("on")?)?
            .offset(args.optional_number("offset", &self.scope)?.unwrap_or(0.0));
        let transform = reflection(&frame);
        let copy = moved(self.active("mirror")?, transform);
        let label = label_of(line);
        self.copy_groups(None, transform, &label);
        let body = self.body_name();
        let originals: Vec<GroupEntry> = self
            .groups
            .0
            .iter()
            .filter(|e| e.body == body)
            .filter(|e| e.label != label)
            .cloned()
            .collect();
        originals.iter().for_each(|entry| {
            self.groups.record(
                &entry.body,
                &entry.label,
                &entry.group,
                moved_surface(&entry.surface, transform),
            )
        });
        self.merge(&label, copy, Combine::Add)?;
        self.describe_solid()
    }

    pub(crate) fn op_repeat(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["of"], &["count", "step", "angle", "along"], false)?;
        let of = args.text("of")?;
        let tools = self.tools.get(of).cloned().ok_or_else(|| {
            let mut known: Vec<&String> = self.tools.keys().collect();
            known.sort();
            anyhow!("`{of}` made no material to repeat; lines that did: {known:?}")
        })?;
        let count = args.number("count", &self.scope)?;
        if count < 2.0 || count.fract() != 0.0 {
            bail!("count must be a whole number of at least 2, got {count}");
        }
        let count = count as usize;
        let frame = self.sketch_frame();
        let along = match args.values.get("along").copied() {
            None => None,
            Some("path") => {
                let path = self
                    .path
                    .clone()
                    .ok_or_else(|| anyhow!("`along=path` needs a `path` or `helix` first"))?;
                let open = !matches!(&path, super::SweepPath::Polyline { points, .. } if (points[0] - points[points.len() - 1]).magnitude() < 1.0e-9);
                let steps = if open { count - 1 } else { count };
                let (start, start_tangent) = path.at(0.0)?;
                let stations = (1..count)
                    .map(|k| {
                        let (point, tangent) = path.at(k as f64 / steps as f64)?;
                        let turn = start_tangent.cross(tangent);
                        let rotation = if turn.magnitude() < 1.0e-12 {
                            Matrix4::identity()
                        } else {
                            Matrix4::from_axis_angle(
                                turn.normalize(),
                                Rad(start_tangent.dot(tangent).clamp(-1.0, 1.0).acos()),
                            )
                        };
                        Ok(Matrix4::from_translation(point.to_vec())
                            * rotation
                            * Matrix4::from_translation(-start.to_vec()))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Some(stations)
            }
            Some(other) => bail!("`along=` takes `path`, got `{other}`"),
        };
        let place: Box<dyn Fn(usize) -> Matrix4> = match (
            args.optional_point("step", &self.scope)?,
            args.optional_number("angle", &self.scope)?,
        ) {
            _ if along.is_some() => {
                if args.has("step") || args.has("angle") {
                    bail!("give `along=`, `step=` or `angle=`, only one");
                }
                let stations = along.expect("checked");
                Box::new(move |k| stations[k - 1])
            }
            (Some((x, y)), None) => {
                let step = frame.x * x + frame.y * y;
                Box::new(move |k| Matrix4::from_translation(step * k as f64))
            }
            (None, Some(angle)) => {
                let delta = if (angle.abs() - 360.0).abs() < 1.0e-9 {
                    angle / count as f64
                } else {
                    angle / (count - 1) as f64
                };
                Box::new(move |k| {
                    about(
                        frame.origin,
                        Matrix4::from_axis_angle(frame.normal, Deg(delta * k as f64)),
                    )
                })
            }
            _ => bail!(
                "give `step=x,y` for a row, `angle=` for a circle about the workplane normal, or `along=path`"
            ),
        };
        let label = label_of(line);
        for k in 1..count {
            let transform = place(k);
            self.copy_groups(Some(of), transform, of);
            for (combine, tool) in &tools {
                self.merge(&label, moved(tool, transform), *combine)?;
            }
        }
        self.describe_solid()
    }

    fn place_or_copy(&mut self, line: &Line, transform: Matrix4, copy: bool) -> Result<String> {
        if copy {
            let duplicate = moved(self.active(&line.op)?, transform);
            let label = label_of(line);
            self.copy_groups(None, transform, &label);
            self.merge(&label, duplicate, Combine::Add)?;
        } else {
            self.transform_all(transform)?;
        }
        self.describe_solid()
    }

    pub(crate) fn op_move(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["by", "copy"], &[], false)?;
        let by = point3(args.text("by")?, &self.scope)?;
        let copy = copy_flag(&args)?;
        if self.has_mesh() {
            if copy {
                bail!("`move copy` copies solids; a mesh moves its selection");
            }
            return self.mesh_move([by.x, by.y, by.z]);
        }
        self.place_or_copy(line, Matrix4::from_translation(by.to_vec()), copy)
    }

    pub(crate) fn op_rotate(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["angle", "copy"], &["axis", "about"], false)?;
        let copy = copy_flag(&args)?;
        let angle = args.number("angle", &self.scope)?;
        let named = args
            .values
            .get("axis")
            .and_then(|name| self.axes.get(*name))
            .copied();
        let direction = match named {
            Some((_, direction)) => direction,
            None => axis(args.values.get("axis").copied().unwrap_or("z"))?,
        };
        let center = match (args.values.get("about"), named) {
            (Some(_), Some(_)) => {
                bail!("a datum axis already passes through a point; drop `about=`")
            }
            (Some(text), None) => point3(text, &self.scope)?,
            (None, Some((through, _))) => through,
            (None, None) => Point3::origin(),
        };
        if self.has_mesh() {
            if copy {
                bail!("`rotate copy` copies solids; a mesh turns its selection");
            }
            let about =
                (args.has("about") || named.is_some()).then_some([center.x, center.y, center.z]);
            return self.mesh_rotate([direction.x, direction.y, direction.z], angle, about);
        }
        self.place_or_copy(
            line,
            about(center, Matrix4::from_axis_angle(direction, Deg(angle))),
            copy,
        )
    }

    pub(crate) fn op_scale(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["factor"], &["about"], false)?;
        let text = args.text("factor")?;
        let factors = if text.contains(',') {
            let p = point3(text, &self.scope)?;
            [p.x, p.y, p.z]
        } else {
            [args.number("factor", &self.scope)?; 3]
        };
        factors
            .iter()
            .try_for_each(|f| positive(*f, "scale factor").map(|_| ()))?;
        let given = args
            .values
            .get("about")
            .map(|text| point3(text, &self.scope))
            .transpose()?;
        if self.has_mesh() {
            return self.mesh_scale(factors, given.map(|c| [c.x, c.y, c.z]));
        }
        let center = given.unwrap_or_else(Point3::origin);
        self.transform_all(about(
            center,
            Matrix4::from_nonuniform_scale(factors[0], factors[1], factors[2]),
        ))?;
        self.describe_solid()
    }

    pub(crate) fn op_split(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["on"], &["offset", "keep"], false)?;
        let frame = self
            .plane_from(args.text("on")?)?
            .offset(args.optional_number("offset", &self.scope)?.unwrap_or(0.0));
        let side = match args.values.get("keep").copied() {
            Some("below") => -1.0,
            Some("above") => 1.0,
            _ => {
                bail!("`split` needs keep=below or keep=above (below is against the plane normal)")
            }
        };
        let solid = self.active("split")?;
        let bounds = geometry::bounds(solid);
        let size = bounds.diameter() * 1.5 + 1.0;
        let (u, v) = frame.local(bounds.center());
        let block = Profile::Rect {
            center: (u, v),
            width: size,
            height: size,
            radius: 0.0,
        };
        let shapes = loops(&frame, std::slice::from_ref(&block))?;
        let tool = prism(
            &frame,
            &shapes,
            &[0],
            std::slice::from_ref(&block),
            frame.normal * (-side * size),
            (0.0, 0.0),
        )?;
        let label = label_of(line);
        let mut cut_face = Surface::Plane(Plane::new(
            frame.origin,
            frame.origin + frame.x,
            frame.origin + frame.y,
        ));
        if side > 0.0 {
            cut_face.invert();
        }
        self.groups
            .record(&self.body_name(), &label, "cut", cut_face);
        self.merge(&label, tool, Combine::Remove)?;
        self.describe_solid()
    }

    pub(crate) fn op_color(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["colour"], &["opacity", "part", "gloss", "soft"], false)?;
        let colour = self.colour_of(args.text("colour")?)?;
        let opacity = opacity_of(&args, &self.scope)?;
        let gloss = args.optional_number("gloss", &self.scope)?;
        if let Some(g) = gloss
            && !(0.0..=1.0).contains(&g)
        {
            bail!("gloss goes from 0 (matte) to 1 (wet), got {g}");
        }
        let soft = args.optional_number("soft", &self.scope)?;
        if let Some(s) = soft
            && s < 0.0
        {
            bail!("soft must be 0 or more, got {s}");
        }
        if let Some(part) = args.values.get("part") {
            return self.colour_mesh_part(part, colour, gloss, soft);
        }
        if gloss.is_some() || soft.is_some() {
            bail!("`gloss=` and `soft=` go with `part=` on a mesh body");
        }
        let name = self.body_name();
        self.colours.insert(name.clone(), colour);
        self.opacities.insert(name.clone(), opacity);
        Ok(format!(
            "body `{name}` is {:.2},{:.2},{:.2}{}",
            colour[0],
            colour[1],
            colour[2],
            see_through(opacity)
        ))
    }

    pub(crate) fn op_material(&mut self, line: &Line) -> Result<String> {
        let name = if self.assembly {
            let part = line
                .positional
                .first()
                .ok_or_else(|| anyhow!("write `material part name [density=g/cm³]`"))?
                .clone();
            if !self.body_names().contains(&part) {
                bail!("no part called `{part}`; parts are {:?}", self.body_names());
            }
            part
        } else {
            self.body_name()
        };
        let wanted: &[&str] = if self.assembly { &["part"] } else { &[] };
        self.material_for(line, name, wanted)
    }

    pub(crate) fn material_for(
        &mut self,
        line: &Line,
        name: String,
        wanted: &[&'static str],
    ) -> Result<String> {
        let args = Args::new(
            line,
            wanted,
            &["density", "E", "nu", "yield", "k", "alpha", "cp"],
            true,
        )?;
        let label = match args.rest.as_slice() {
            [] => None,
            [one] => Some(one.to_string()),
            _ => bail!("give one material name, like `material steel`"),
        };
        self.ensure_library()?;
        let entry = label
            .as_deref()
            .and_then(|l| self.library.get(&super::materials::slug(l)).cloned());
        let known = label.as_deref().and_then(stock_of);
        let listed = entry
            .as_ref()
            .and_then(|e| e.density)
            .or(known.map(|s| s.density));
        let density = match (
            args.optional_number("density", &self.scope)?,
            listed,
            &label,
        ) {
            (Some(d), _, _) if d <= 0.0 => bail!("density must be above zero, not {d}"),
            (Some(d), _, _) => d,
            (None, Some(d), _) => d,
            (None, None, Some(label)) => {
                let mut names: Vec<String> = MATERIALS.iter().map(|m| m.name.to_string()).collect();
                names.extend(self.library.keys().cloned());
                names.sort();
                names.dedup();
                bail!(
                    "no density known for `{label}`; give `density=` in g/cm³, add it to a materials.toml, or use one of {}",
                    names.join(", ")
                )
            }
            (None, None, None) => bail!("write `material steel` or `material name density=1.2`"),
        };
        let mut mechanics = match (&entry, known) {
            (Some(e), Some(stock)) if e.density.is_none() => e.mechanics.over(stock.mechanics()),
            (Some(e), _) => e.mechanics,
            (None, Some(stock)) => stock.mechanics(),
            (None, None) => Mechanics::default(),
        };
        let finish = entry.as_ref().and_then(|e| e.finish.clone());
        if let Some(colour) = finish.as_ref().and_then(|f| f.colour) {
            self.colours.entry(name.clone()).or_insert(colour);
        }
        let axes = entry.as_ref().and_then(|e| e.axes);
        for (key, slot) in [
            ("E", &mut mechanics.young),
            ("nu", &mut mechanics.poisson),
            ("yield", &mut mechanics.strength),
            ("k", &mut mechanics.conductivity),
            ("alpha", &mut mechanics.expansion),
            ("cp", &mut mechanics.heat_capacity),
        ] {
            if let Some(value) = args.optional_number(key, &self.scope)? {
                let fine = if key == "nu" {
                    (0.0..0.5).contains(&value)
                } else {
                    value > 0.0
                };
                if !fine {
                    bail!("`{key}={value}` is out of range");
                }
                *slot = Some(value);
            }
        }
        let label = label.unwrap_or_else(|| format!("{density} g/cm³"));
        self.materials.insert(
            name.clone(),
            Material {
                name: label.clone(),
                density,
                mechanics,
                axes,
                finish,
            },
        );
        let from = entry
            .map(|e| format!(" from {}", e.source.display()))
            .unwrap_or_default();
        Ok(format!("`{name}` is {label}, {density} g/cm³{from}"))
    }

    pub(crate) fn ensure_library(&mut self) -> Result<()> {
        if !self.library_loaded {
            for (name, entry) in super::materials::discovered(self.dir.as_deref())? {
                self.library.entry(name).or_insert(entry);
            }
            self.library_loaded = true;
        }
        Ok(())
    }

    pub(crate) fn load_library(&mut self, file: &std::path::Path) -> Result<String> {
        if !crate::host::is_file(file) {
            bail!("no file {}", file.display());
        }
        self.ensure_library()?;
        let entries = super::materials::load(file)?;
        let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
        for entry in entries {
            let entry = match self.library.remove(&entry.name) {
                Some(base) => entry.over(base),
                None => entry,
            };
            self.library.insert(entry.name.clone(), entry);
        }
        Ok(format!("materials {}", names.join(", ")))
    }

    pub(crate) fn op_materials(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["file"], &[], false)?;
        let file = self.relative(args.text("file")?);
        self.load_library(&file)
    }

    pub fn named_solids(&self) -> Vec<(String, &Solid)> {
        self.bodies
            .iter()
            .map(|(n, s)| (n.clone(), s))
            .chain(self.solid.as_ref().map(|s| (self.body_name(), s)))
            .collect()
    }

    pub(crate) fn colour_of(&self, text: &str) -> Result<[f64; 3]> {
        let colour = if text.starts_with('#') {
            colour_text(text).ok_or_else(|| anyhow!("`{text}` is not a colour; write #rrggbb"))?
        } else if text.contains(',') {
            let p = point3(text, &self.scope)?;
            let rgb = [p.x, p.y, p.z];
            if rgb.iter().any(|c| !(0.0..=1.0).contains(c)) {
                bail!("colour parts go from 0 to 1, got {text}");
            }
            rgb
        } else {
            colour_text(text).ok_or_else(|| {
                anyhow!("`{text}` is not a colour; use a name like red or grey, #rrggbb, or r,g,b from 0 to 1")
            })?
        };
        Ok(colour)
    }

    pub(crate) fn op_body(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["name"], &[], false)?;
        self.require_empty_sketch("body")?;
        let name = args.text("name")?.to_string();
        if self.body == name
            || self.bodies.iter().any(|(n, _)| *n == name)
            || self.mesh_bodies.iter().any(|(n, _)| *n == name)
        {
            bail!("there is already a body called `{name}`");
        }
        self.finish_mesh_body()?;
        if let Some(solid) = self.solid.take() {
            let previous = if self.body.is_empty() {
                "main".to_string()
            } else {
                self.body.clone()
            };
            self.bodies.push((previous, solid));
        }
        self.body = name;
        let finished = self.bodies.len() + self.mesh_bodies.len();
        Ok(format!(
            "{finished} finished bod{}; new body `{}`",
            if finished == 1 { "y" } else { "ies" },
            self.body
        ))
    }

    pub(crate) fn op_import(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(line, &["file", "as"], &["solid"], false)?;
        let file = std::path::PathBuf::from(args.text("file")?);
        let path = match &self.dir {
            Some(dir) if file.is_relative() => dir.join(&file),
            _ => file,
        };
        match args.values.get("as").copied() {
            Some("mesh") => return self.import_mesh(line, &path),
            Some(other) => {
                bail!("`import file` reads a solid; `import file mesh` reads a mesh, not `{other}`")
            }
            None => {}
        }
        let stl = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("stl"));
        let pick = match args.optional_number("solid", &self.scope)? {
            Some(n) if n.fract() == 0.0 && n >= 1.0 => Some(n as usize - 1),
            Some(_) => bail!("`solid` must be a whole number from 1"),
            None => None,
        };
        let found = if stl {
            vec![(String::new(), super::import::stl_solid(&path)?, None, 0)]
        } else {
            read_step(&path, pick)
                .with_context(|| format!("importing {}", path.display()))?
                .into_iter()
                .map(|part| (part.name, part.solid, part.failure, part.filled))
                .collect()
        };
        let listing = || {
            found
                .iter()
                .enumerate()
                .map(|(k, (name, solid, ..))| {
                    let b = geometry::bounds(solid);
                    let (lo, hi) = (b.min(), b.max());
                    format!(
                        "{}: `{name}` {:.1} x {:.1} x {:.1}",
                        k + 1,
                        hi.x - lo.x,
                        hi.y - lo.y,
                        hi.z - lo.z
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let (solid, failure, filled) = match (pick, found.as_slice()) {
            (_, [(_, only, failure, filled)]) => (only.clone(), failure.clone(), *filled),
            (Some(_), _) => bail!(
                "`solid` must be 1 to {}; the file holds {}",
                found.len(),
                listing()
            ),
            (None, _) => bail!(
                "the file holds {} solids, pick one with `solid=`: {}",
                found.len(),
                listing()
            ),
        };
        let label = label_of(line);
        let groups = select::faces(&solid)
            .iter()
            .map(|face| ("faces", face.oriented_surface()))
            .collect();
        self.record(&label, groups);
        self.merge(&label, solid, Combine::Add)?;
        let mut summary = self.describe_solid()?;
        if filled > 0 {
            let are = if filled == 1 { "is" } else { "are" };
            summary += &format!("; {filled} of its cavities gcad cannot read {are} left solid");
        }
        Ok(match failure {
            Some(why) => {
                format!("{summary}; a box stands in for the solid, which gcad cannot read: {why}")
            }
            None => summary,
        })
    }
}

type StepTable = monstertruck::step::load::Table;

fn placed_breps(
    table: &StepTable,
    raw: &super::step_raw::Raw,
) -> Vec<(u64, String, String, Matrix4)> {
    let edges = raw.edge_matrices();
    let in_millimetres = |attributes: &monstertruck::step::common::PartAttributes,
                          step: Matrix4| {
        let key = (
            attributes.id.clone(),
            attributes.name.clone(),
            attributes.description.clone(),
        );
        edges
            .get(&key)
            .and_then(|found| {
                found
                    .iter()
                    .find(|(native, _)| super::step_raw::close(native, &step))
                    .map(|(_, millimetres)| *millimetres)
            })
            .unwrap_or(step)
    };
    let Ok(assembly) = table.step_assy() else {
        return Vec::new();
    };
    let mut placed = Vec::new();
    for top in assembly.top_nodes() {
        for path in assembly.maximal_paths_iter(top.index()) {
            let Some(matrix) = path
                .edges()
                .iter()
                .try_fold(Matrix4::identity(), |m, edge| {
                    Matrix4::try_from(edge.matrix())
                        .ok()
                        .map(|step| m * in_millimetres(edge.attributes(), step))
                })
            else {
                continue;
            };
            let Some(node) = path.nodes().last().copied() else {
                continue;
            };
            let mut ids: Vec<u64> = node
                .shape()
                .iter()
                .copied()
                .filter(|id| table.manifold_solid_brep.contains_key(id))
                .collect();
            if let Some(representation) = table.shape_representation_of_node(node.entity()) {
                ids.extend(
                    table
                        .solids_via_shape_relationship(representation)
                        .0
                        .into_iter()
                        .filter(|id| table.manifold_solid_brep.contains_key(id)),
                );
            }
            ids.sort_unstable();
            ids.dedup();
            let name = path
                .edges()
                .last()
                .map(|edge| edge.attributes().name.clone())
                .filter(|name| !name.is_empty() && !name.starts_with("=>"))
                .unwrap_or_else(|| node.attributes().name.clone());
            let product = node.attributes().name.clone();
            placed.extend(
                ids.into_iter()
                    .map(|id| (id, name.clone(), product.clone(), matrix)),
            );
        }
    }
    placed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| natural(&a.1, &b.1)));
    placed
}

fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| {
        let digits = s.len() - s.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        let (head, tail) = s.split_at(s.len() - digits);
        (head.to_string(), tail.parse::<u64>().unwrap_or(0))
    };
    split(a).cmp(&split(b))
}

pub(crate) struct StepSolid {
    pub name: String,
    pub product: String,
    pub matrix: Matrix4,
    pub solid: Solid,
    pub failure: Option<String>,
    pub filled: usize,
}

type StepTrimmedSolid = monstertruck::topology::compress::CompressedTrimmedSolid<
    Point3,
    monstertruck::step::load::step_geometry::Curve3D,
    monstertruck::step::load::step_geometry::Surface,
    monstertruck::step::load::step_geometry::StepParameterCurve,
>;

fn trimmed_bounds(solid: &StepTrimmedSolid) -> BoundingBox<Point3> {
    solid
        .boundaries
        .iter()
        .flat_map(|shell| {
            let ends = shell.vertices.iter().copied();
            let along = shell.edges.iter().flat_map(|edge| {
                let range = edge.curve.try_range_tuple();
                range
                    .into_iter()
                    .flat_map(move |(start, end)| {
                        (0..=8).map(move |k| start + (end - start) * k as f64 / 8.0)
                    })
                    .map(|t| edge.curve.subs(t))
            });
            ends.chain(along)
        })
        .collect()
}

fn outside_the_outer(compressed: StepTrimmedSolid) -> Vec<StepTrimmedSolid> {
    use monstertruck::topology::compress::CompressedTrimmedSolid;
    let mut shells = compressed.boundaries.into_iter();
    let Some(outer) = shells.next() else {
        return Vec::new();
    };
    let shell_bounds = |shell: &_| {
        trimmed_bounds(&CompressedTrimmedSolid {
            boundaries: vec![Clone::clone(shell)],
        })
    };
    let around = shell_bounds(&outer);
    let gap = around.diameter() * 1.0e-9;
    let apart = |b: &BoundingBox<Point3>| {
        (0..3).any(|k| b.min()[k] < around.min()[k] - gap || b.max()[k] > around.max()[k] + gap)
    };
    let (outside, inside): (Vec<_>, Vec<_>) = shells.partition(|void| apart(&shell_bounds(void)));
    std::iter::once(CompressedTrimmedSolid {
        boundaries: std::iter::once(outer).chain(inside).collect(),
    })
    .chain(outside.into_iter().map(|void| CompressedTrimmedSolid {
        boundaries: vec![void],
    }))
    .collect()
}

fn healed_without_bad_voids(
    compressed: StepTrimmedSolid,
) -> monstertruck::topology::Result<(
    monstertruck::topology::trimmed::TrimmedSolid<
        Point3,
        monstertruck::step::load::step_geometry::Curve3D,
        monstertruck::step::load::step_geometry::Surface,
        monstertruck::step::load::step_geometry::StepParameterCurve,
    >,
    usize,
)> {
    use monstertruck::healing::extract_healed_trimmed_solid as heal;
    use monstertruck::topology::compress::CompressedTrimmedSolid;
    let error = match heal(compressed.clone(), 1.0e-3) {
        Ok(solid) => return Ok((solid, 0)),
        Err(error) => error,
    };
    let heals = |shell: &_| {
        heal(
            CompressedTrimmedSolid {
                boundaries: vec![Clone::clone(shell)],
            },
            1.0e-3,
        )
        .is_ok()
    };
    let mut shells = compressed.boundaries.into_iter();
    let Some(outer) = shells.next().filter(|outer| heals(outer)) else {
        return Err(error);
    };
    let (kept, dropped): (Vec<_>, Vec<_>) = shells.partition(|void| heals(void));
    if dropped.is_empty() {
        return Err(error);
    }
    let boundaries = std::iter::once(outer).chain(kept).collect();
    heal(CompressedTrimmedSolid { boundaries }, 1.0e-3).map(|solid| (solid, dropped.len()))
}

fn stand_in(low: Point3, high: Point3) -> Result<Solid> {
    let size = high - low;
    let least = size.x.max(size.y).max(size.z) * 1.0e-3;
    let size = Vector3::new(size.x.max(least), size.y.max(least), size.z.max(least));
    if size.x <= 0.0 {
        bail!("it has no points to box");
    }
    let v = builder::vertex(low);
    let e = builder::extrude(&v, Vector3::unit_x() * size.x);
    let f = builder::extrude(&e, Vector3::unit_y() * size.y);
    Ok(oriented(builder::extrude(&f, Vector3::unit_z() * size.z)))
}

fn ident(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_').to_string();
    if out.is_empty() {
        "solid".to_string()
    } else {
        out
    }
}

pub(crate) fn step_model(path: &std::path::Path) -> Result<Model> {
    let found = read_step(path, None).with_context(|| format!("importing {}", path.display()))?;
    let mut model = Model {
        dir: path.parent().map(std::path::Path::to_path_buf),
        ..Default::default()
    };
    for part in found {
        let base = ident(&part.product);
        let mut name = base.clone();
        let mut n = 2;
        while model.bodies.iter().any(|(b, _)| *b == name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        model.sources.insert(
            name.clone(),
            super::Source {
                file: path.to_path_buf(),
                body: ident(&part.product),
                vars: Vec::new(),
            },
        );
        model.placements.insert(name.clone(), part.matrix);
        if let Some(why) = part.failure {
            model.stand_ins.insert(name.clone(), why);
        }
        if part.filled > 0 {
            model.filled.insert(name.clone(), part.filled);
        }
        model.bodies.push((name, part.solid));
    }
    Ok(model)
}

fn step_curve(curve: &monstertruck::step::load::step_geometry::Curve3D) -> Option<Curve> {
    if let Ok(curve) = Curve::try_from(curve) {
        return Some(curve);
    }
    const SAMPLES: usize = 33;
    let (start, end) = curve.try_range_tuple()?;
    let points: Vec<Point3> = (0..SAMPLES)
        .map(|i| curve.subs(start + (end - start) * i as f64 / (SAMPLES - 1) as f64))
        .collect();
    let params = super::skin::uniform(SAMPLES);
    let knots = super::skin::knots(&params, 3);
    let control = super::skin::interpolate(&points, &params, &knots).ok()?;
    Some(Curve::BsplineCurve(BsplineCurve::new(knots, control)))
}

fn step_surface(surface: &monstertruck::step::load::step_geometry::Surface) -> Option<Surface> {
    use monstertruck::step::load::step_geometry::{
        Conic3D, Curve3D, Surface as StepSurface, SweepSurface,
    };
    if let Ok(surface) = Surface::try_from(surface) {
        return Some(surface);
    }
    let StepSurface::SweepSurface(SweepSurface::RevolutionSurface(revolved)) = surface else {
        return None;
    };
    let Curve3D::Conic(Conic3D::Ellipse(ellipse)) = revolved.entity().entity_curve() else {
        return None;
    };
    let place = *revolved.transform() * *ellipse.transform();
    let centre = Point3::from_homogeneous(place * Vector4::unit_w());
    let (x, y) = (
        (place * Vector4::unit_x()).truncate(),
        (place * Vector4::unit_y()).truncate(),
    );
    let origin = revolved
        .transform()
        .transform_point(revolved.entity().origin());
    let axis = revolved
        .transform()
        .transform_vector(revolved.entity().axis())
        .normalize();
    let small = x.magnitude();
    let normal = x.cross(y).normalize();
    let hub = origin + axis * (centre - origin).dot(axis);
    let large = (centre - hub).magnitude();
    let tiny = 1.0e-9 * (large + small);
    if (y.magnitude() - small).abs() > tiny
        || x.dot(y).abs() > tiny * small
        || normal.dot(axis).abs() > 1.0e-9
        || normal.dot(centre - hub).abs() > tiny
        || large <= small
    {
        return None;
    }
    let across = (centre - hub) / large;
    let frame = Matrix4::from_cols(
        across.extend(0.0),
        axis.cross(across).extend(0.0),
        axis.extend(0.0),
        hub.to_homogeneous(),
    );
    let mut torus =
        monstertruck::geometry::prelude::Processor::new(Torus::new(Point3::origin(), large, small));
    torus.transform_by(frame);
    let (u, v) = revolved.try_range_tuple();
    let ((u0, u1), (v0, v1)) = (u?, v?);
    let (u, v) = ((u0 + u1) / 2.0, (v0 + v1) / 2.0);
    let point = revolved.subs(u, v);
    let (tu, tv) = torus.search_nearest_parameter(point, None, 100)?;
    if revolved.normal(u, v).dot(torus.normal(tu, tv)) < 0.0 {
        torus.invert();
    }
    Some(Surface::ToroidalSurface(torus))
}

fn read_step(path: &std::path::Path, pick: Option<usize>) -> Result<Vec<StepSolid>> {
    let text = crate::host::read(path)?;
    let table = StepTable::from_step_bytes(&text).map_err(|error| anyhow!("{error}"))?;
    let raw = super::step_raw::Raw::parse(&String::from_utf8_lossy(&text));
    let mut placed = placed_breps(&table, &raw);
    if placed.is_empty() {
        placed = raw
            .placed()
            .into_iter()
            .map(|p| (p.id, p.name, p.product, p.matrix))
            .collect();
        placed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| natural(&a.1, &b.1)));
    }
    if placed.is_empty() && table.manifold_solid_brep.is_empty() {
        bail!("the file has no solids");
    }
    if placed.is_empty() {
        let mut ids: Vec<u64> = table.manifold_solid_brep.keys().copied().collect();
        ids.sort_unstable();
        placed = ids
            .into_iter()
            .map(|id| {
                let label = table.manifold_solid_brep[&id].label.clone();
                (id, label.clone(), label, Matrix4::identity())
            })
            .collect();
    } else {
        for (id, holder) in &table.manifold_solid_brep {
            if !placed.iter().any(|(p, ..)| p == id) {
                placed.push((
                    *id,
                    holder.label.clone(),
                    holder.label.clone(),
                    Matrix4::identity(),
                ));
            }
        }
    }
    let chosen: Vec<_> = match pick {
        Some(index) if index < placed.len() => vec![placed[index].clone()],
        _ => placed,
    };
    let mut converted: std::collections::HashMap<u64, Vec<(Solid, Option<String>, usize)>> =
        std::collections::HashMap::new();
    let compressed_of = |id: u64| -> Result<StepTrimmedSolid> {
        Ok(
            match (
                table.manifold_solid_brep.get(&id),
                table.shell_based_surface_model.get(&id),
            ) {
                (Some(holder), _) => table
                    .to_compressed_trimmed_solid(holder)
                    .map_err(|error| anyhow!("{error}"))?,
                (None, Some(model)) => {
                    let shells = table
                        .to_compressed_trimmed_shells(model)
                        .map_err(|error| anyhow!("{error}"))?;
                    if shells.len() != 1 {
                        bail!("a surface model of {} shells", shells.len());
                    }
                    monstertruck::topology::compress::CompressedTrimmedSolid { boundaries: shells }
                }
                (None, None) => bail!("{}", raw.kind(id).to_lowercase().replace('_', " ")),
            },
        )
    };
    let solid_of = |compressed: StepTrimmedSolid, scale: f64| -> Result<(Solid, usize)> {
        let (healed, filled) = healed_without_bad_voids(compressed).map_err(|error| {
            anyhow!(
                "healing fails ({})",
                format!("{error}").trim_end_matches('.')
            )
        })?;
        let solid = healed
            .erase_trims()
            .try_mapped(|point| Some(*point), step_curve, step_surface)
            .ok_or_else(|| anyhow!("a curve or surface type gcad has no kernel form for"))?;
        let closed = solid.boundaries().iter().all(|shell| {
            shell.shell_condition() == monstertruck::topology::shell::ShellCondition::Closed
        });
        let volume = geometry::volume(&solid);
        if !closed || !volume.is_finite() || volume == 0.0 {
            bail!("a surface that does not close around a volume");
        }
        let solid = if scale == 1.0 {
            solid
        } else {
            builder::transformed(&solid, Matrix4::from_scale(scale))
        };
        Ok((oriented(solid), filled))
    };
    let pieces = |id: u64, name: &str| -> Result<Vec<(Solid, Option<String>, usize)>> {
        if let Some(polygons) = raw.polygons(id) {
            return super::import::connected_polygons(&polygons)
                .into_iter()
                .map(|part| match super::import::polygon_solid(&part) {
                    Ok(solid) => Ok((solid, None, 0)),
                    Err(error) => {
                        let points: BoundingBox<Point3> =
                            part.iter().flatten().flatten().copied().collect();
                        Ok((
                            stand_in(points.min(), points.max())?,
                            Some(format!("{error:#}")),
                            0,
                        ))
                    }
                })
                .collect();
        }
        let scale = raw.scale_of(id);
        let groups = match compressed_of(id) {
            Ok(compressed) => outside_the_outer(compressed),
            Err(error) => {
                let (low, high) = raw.bounds(id).ok_or_else(|| anyhow!("`{name}`: {error}"))?;
                return Ok(vec![(stand_in(low, high)?, Some(format!("{error}")), 0)]);
            }
        };
        let alone = groups.len() == 1;
        groups
            .into_iter()
            .map(|group| {
                let bounds = trimmed_bounds(&group);
                match solid_of(group, scale) {
                    Ok((solid, filled)) => Ok((solid, None, filled)),
                    Err(error) => {
                        let (low, high) = match raw.bounds(id) {
                            Some(found) if alone => found,
                            _ => (bounds.min() * scale, bounds.max() * scale),
                        };
                        Ok((stand_in(low, high)?, Some(format!("{error}")), 0))
                    }
                }
            })
            .collect()
    };
    let mut out = Vec::new();
    for (id, name, product, matrix) in chosen {
        let found = match converted.get(&id) {
            Some(found) => found.clone(),
            None => {
                let found = pieces(id, &name)?;
                converted.insert(id, found.clone());
                found
            }
        };
        for (solid, failure, filled) in found {
            let placed = if matrix == Matrix4::identity() {
                solid
            } else {
                super::solids::placed(&solid, matrix)
            };
            out.push(StepSolid {
                name: name.clone(),
                product: product.clone(),
                matrix,
                solid: placed,
                failure,
                filled,
            });
        }
    }
    Ok(out)
}

pub(crate) fn colour_text(text: &str) -> Option<[f64; 3]> {
    if let Some(hex) = text.strip_prefix('#') {
        let channel = |i: usize| {
            hex.get(i..i + 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .map(|v| v as f64 / 255.0)
        };
        return match (hex.len(), channel(0), channel(2), channel(4)) {
            (6, Some(r), Some(g), Some(b)) => Some([r, g, b]),
            _ => None,
        };
    }
    Some(match text {
        "red" => [0.85, 0.1, 0.1],
        "green" => [0.1, 0.6, 0.2],
        "blue" => [0.1, 0.3, 0.85],
        "yellow" => [0.95, 0.8, 0.1],
        "orange" => [0.95, 0.5, 0.1],
        "purple" => [0.5, 0.2, 0.7],
        "cyan" => [0.1, 0.7, 0.8],
        "magenta" => [0.85, 0.2, 0.6],
        "black" => [0.05, 0.05, 0.05],
        "white" => [0.95, 0.95, 0.95],
        "grey" | "gray" => [0.5, 0.5, 0.5],
        "silver" => [0.75, 0.75, 0.78],
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Material {
    pub name: String,
    pub density: f64,
    pub mechanics: Mechanics,
    pub axes: Option<super::materials::Axes>,
    #[serde(default)]
    pub finish: Option<super::materials::Finish>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Mechanics {
    pub young: Option<f64>,
    pub poisson: Option<f64>,
    pub strength: Option<f64>,
    pub conductivity: Option<f64>,
    pub expansion: Option<f64>,
    #[serde(default)]
    pub heat_capacity: Option<f64>,
    pub printed: bool,
}

impl Mechanics {
    pub(crate) fn over(self, base: Mechanics) -> Mechanics {
        Mechanics {
            young: self.young.or(base.young),
            poisson: self.poisson.or(base.poisson),
            strength: self.strength.or(base.strength),
            conductivity: self.conductivity.or(base.conductivity),
            expansion: self.expansion.or(base.expansion),
            heat_capacity: self.heat_capacity.or(base.heat_capacity),
            printed: self.printed || base.printed,
        }
    }
}

pub struct Stock {
    pub name: &'static str,
    pub density: f64,
    pub young: f64,
    pub poisson: f64,
    pub strength: f64,
    pub conductivity: f64,
    pub expansion: f64,
    pub heat_capacity: f64,
    pub printed: bool,
}

const fn stock(
    name: &'static str,
    density: f64,
    [
        young,
        poisson,
        strength,
        conductivity,
        expansion,
        heat_capacity,
    ]: [f64; 6],
    printed: bool,
) -> Stock {
    Stock {
        name,
        density,
        young,
        poisson,
        strength,
        conductivity,
        expansion,
        heat_capacity,
        printed,
    }
}

pub const MATERIALS: &[Stock] = &[
    stock(
        "steel",
        7.85,
        [200.0, 0.29, 250.0, 50.0, 12.0, 490.0],
        false,
    ),
    stock(
        "stainless",
        8.0,
        [193.0, 0.29, 215.0, 16.0, 17.0, 500.0],
        false,
    ),
    stock(
        "aluminium",
        2.7,
        [69.0, 0.33, 240.0, 167.0, 23.6, 900.0],
        false,
    ),
    stock(
        "aluminum",
        2.7,
        [69.0, 0.33, 240.0, 167.0, 23.6, 900.0],
        false,
    ),
    stock("brass", 8.5, [97.0, 0.31, 124.0, 115.0, 20.5, 380.0], false),
    stock(
        "copper",
        8.96,
        [117.0, 0.34, 70.0, 390.0, 17.0, 385.0],
        false,
    ),
    stock(
        "titanium",
        4.43,
        [114.0, 0.34, 880.0, 6.7, 8.6, 523.0],
        false,
    ),
    stock("pla", 1.24, [2.8, 0.35, 40.0, 0.13, 68.0, 1800.0], true),
    stock("petg", 1.27, [2.0, 0.38, 35.0, 0.2, 60.0, 1200.0], true),
    stock("abs", 1.04, [2.0, 0.35, 30.0, 0.17, 90.0, 1400.0], true),
    stock("asa", 1.07, [1.9, 0.35, 32.0, 0.17, 95.0, 1300.0], true),
    stock("nylon", 1.14, [1.5, 0.39, 40.0, 0.25, 100.0, 1700.0], true),
    stock("tpu", 1.21, [0.026, 0.48, 8.0, 0.2, 150.0, 1800.0], true),
    stock(
        "polycarbonate",
        1.2,
        [2.3, 0.37, 60.0, 0.2, 68.0, 1200.0],
        false,
    ),
    stock(
        "acrylic",
        1.18,
        [3.2, 0.37, 65.0, 0.19, 75.0, 1470.0],
        false,
    ),
    stock("resin", 1.2, [2.2, 0.35, 50.0, 0.2, 80.0, 1200.0], false),
    stock("wood", 0.6, [10.0, 0.3, 40.0, 0.12, 5.0, 1600.0], false),
    stock("concrete", 2.4, [30.0, 0.2, 30.0, 2.0, 10.0, 880.0], false),
    stock("brick", 1.8, [10.0, 0.2, 10.0, 0.81, 6.0, 840.0], false),
    stock("glass", 2.5, [70.0, 0.22, 40.0, 1.0, 9.0, 840.0], false),
    stock("gypsum", 0.9, [2.0, 0.25, 3.0, 0.25, 14.0, 1090.0], false),
    stock("eps", 0.02, [0.005, 0.1, 0.1, 0.036, 70.0, 1450.0], false),
    stock("wool", 0.04, [0.0005, 0.1, 0.02, 0.04, 3.0, 1030.0], false),
];

pub fn stock_of(name: &str) -> Option<&'static Stock> {
    let name = name.to_ascii_lowercase();
    MATERIALS.iter().find(|m| m.name == name)
}

impl Stock {
    pub fn mechanics(&self) -> Mechanics {
        Mechanics {
            young: Some(self.young),
            poisson: Some(self.poisson),
            strength: Some(self.strength),
            conductivity: Some(self.conductivity),
            expansion: Some(self.expansion),
            heat_capacity: Some(self.heat_capacity),
            printed: self.printed,
        }
    }
}

pub(crate) fn opacity_of(args: &Args, scope: &Scope) -> Result<f64> {
    let opacity = args.optional_number("opacity", scope)?.unwrap_or(1.0);
    if !(opacity > 0.0 && opacity <= 1.0) {
        bail!("opacity must be above 0 and at most 1, got {opacity}");
    }
    Ok(opacity)
}

pub(crate) fn see_through(opacity: f64) -> String {
    if opacity < 1.0 {
        format!(", {:.0}% opaque", opacity * 100.0)
    } else {
        String::new()
    }
}

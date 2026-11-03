use crate::geometry;
use anyhow::{Result, bail};
use monstertruck::mesh::PolygonMesh;
use monstertruck::mesh::stl::{StlType, write};
use monstertruck::modeling::*;
use monstertruck::step::save::{CompleteStepDisplay, StepHeaderDescriptor, StepModels};
use std::fmt::Write as _;

pub type Colour = [f64; 3];

pub fn export(solids: &[&Solid], path: &str) -> Result<()> {
    let parts: Vec<(&Solid, Option<Colour>)> = solids.iter().map(|s| (*s, None)).collect();
    export_coloured(&parts, path)
}

fn fine_mesh(solid: &Solid) -> PolygonMesh {
    geometry::mesh(
        solid,
        (geometry::bounds(solid).diameter() * 2.0e-4).max(0.01),
    )
}

fn merged_mesh(parts: &[(&Solid, Option<Colour>)]) -> PolygonMesh {
    let mut mesh = PolygonMesh::default();
    parts
        .iter()
        .for_each(|(solid, _)| mesh.merge(fine_mesh(solid)));
    mesh
}

pub fn export_coloured(parts: &[(&Solid, Option<Colour>)], path: &str) -> Result<()> {
    let extension = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "step" | "stp" => std::fs::write(path, step_text(parts))?,
        "stl" => write(
            &merged_mesh(parts),
            &mut std::fs::File::create(path)?,
            StlType::Binary,
        )?,
        "obj" => monstertruck::mesh::obj::write(&merged_mesh(parts), std::fs::File::create(path)?)?,
        "3mf" => {
            let named: Vec<(String, &Solid, Option<Colour>)> = parts
                .iter()
                .enumerate()
                .map(|(i, (s, c))| (format!("body{i}"), *s, *c))
                .collect();
            std::fs::write(path, three_mf(&named))?
        }
        "svg" => std::fs::write(path, crate::drawing::drawing(parts))?,
        other => bail!(
            "cannot export `.{other}`, use .step, .stl, .obj, .3mf, .svg or, for sheet metal, .dxf"
        ),
    }
    Ok(())
}

fn step_text(parts: &[(&Solid, Option<Colour>)]) -> String {
    let header = StepHeaderDescriptor {
        organization_system: "gcad".to_string(),
        ..Default::default()
    };
    let tidied: Vec<Solid> = parts.iter().map(|(solid, _)| even_leaders(solid)).collect();
    let compressed: Vec<_> = tidied.iter().map(|solid| solid.compress()).collect();
    let step = CompleteStepDisplay::new(
        compressed.iter().collect::<StepModels<'_, _, _, _>>(),
        header,
    )
    .to_string();
    if parts.iter().all(|(_, colour)| colour.is_none()) {
        return step;
    }
    with_colours(&step, parts)
}

pub struct Instance<'a> {
    pub name: String,
    pub product: usize,
    pub placement: Matrix4,
    pub solid: &'a Solid,
}

pub struct Product {
    pub name: String,
    pub solid: Solid,
    pub colour: Option<Colour>,
}

pub fn assembly(model: &crate::model::Model) -> (Vec<Product>, Vec<Instance<'_>>) {
    let mut products: Vec<(Option<crate::model::Source>, Product)> = Vec::new();
    let mut instances = Vec::new();
    for (name, solid) in model.named_solids() {
        let source = model.sources.get(&name).cloned();
        let placement = model
            .placements
            .get(&name)
            .copied()
            .unwrap_or_else(Matrix4::identity);
        let known = products
            .iter()
            .position(|(s, _)| source.is_some() && *s == source);
        let product = match known {
            Some(index) => index,
            None => {
                let local = placement
                    .invert()
                    .map(|undo| builder::transformed(solid, undo))
                    .unwrap_or_else(|| solid.clone());
                let label = source
                    .as_ref()
                    .map(|s| {
                        let stem = crate::model::file_title(&s.file);
                        if s.body == "main" {
                            stem
                        } else {
                            format!("{stem}.{}", s.body)
                        }
                    })
                    .unwrap_or_else(|| name.clone());
                products.push((
                    source,
                    Product {
                        name: label,
                        solid: local,
                        colour: model.colours.get(&name).copied(),
                    },
                ));
                products.len() - 1
            }
        };
        instances.push(Instance {
            name,
            product,
            placement,
            solid,
        });
    }
    (products.into_iter().map(|(_, p)| p).collect(), instances)
}

pub fn parts_list(
    model: &crate::model::Model,
    file: &std::path::Path,
) -> crate::drawing::PartsList {
    let items = crate::bom::items(model, file);
    let names: Vec<String> = model.named_solids().into_iter().map(|(n, _)| n).collect();
    crate::drawing::PartsList {
        rows: items
            .iter()
            .enumerate()
            .map(|(k, item)| {
                let stem = crate::model::file_title(std::path::Path::new(&item.file));
                vec![
                    (k + 1).to_string(),
                    item.names.len().to_string(),
                    if item.body == "main" {
                        stem
                    } else {
                        format!("{stem}.{}", item.body)
                    },
                    item.material.clone().unwrap_or_else(|| "-".into()),
                    item.grams
                        .map(|g| format!("{g:.2}"))
                        .unwrap_or_else(|| "-".into()),
                ]
            })
            .collect(),
        balloons: items
            .iter()
            .enumerate()
            .filter_map(|(k, item)| {
                let first = item.names.first()?;
                Some((k + 1, names.iter().position(|n| n == first)?))
            })
            .collect(),
    }
}

fn as_mesh(solid: &Solid, name: &str) -> crate::mesh::Mesh {
    let (points, triangles) = geometry::welded(&fine_mesh(solid));
    crate::mesh::Mesh {
        labels: vec![0; points.len()],
        positions: points.iter().map(|p| [p.x, p.y, p.z]).collect(),
        triangles: triangles.iter().map(|t| t.map(|i| i as u32)).collect(),
        parts: vec![name.to_string()],
    }
}

fn export_meshes(model: &crate::model::Model, path: &str) -> Result<()> {
    let extension = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut bodies: Vec<(String, crate::mesh::Mesh, Option<Colour>)> = model
        .named_parts()
        .into_iter()
        .map(|(name, solid, colour)| (name.clone(), as_mesh(solid, &name), colour))
        .collect();
    bodies.extend(
        model
            .coloured_meshes()
            .into_iter()
            .map(|(name, mesh, colour)| (name, (*mesh).clone(), colour)),
    );
    let mut all = crate::mesh::Mesh::default();
    let mut vertex_colours: Vec<[f64; 3]> = Vec::new();
    let shading = model.mesh_shading();
    let mut materials: Vec<(String, Colour, f64)> = Vec::new();
    for (name, mesh, colour) in &bodies {
        let own = model.part_colours_of(name, mesh);
        let gloss = model.part_gloss_of(name, mesh);
        for (k, part) in mesh.parts.iter().enumerate() {
            if !materials.iter().any(|(m, _, _)| m == part) {
                let c = own[k].or(*colour).unwrap_or([0.7, 0.7, 0.7]);
                materials.push((part.clone(), c, gloss[k]));
            }
        }
        match shading.iter().find(|(n, ..)| n == name) {
            Some((_, _, blended, _)) => vertex_colours.extend(blended),
            None => vertex_colours.extend(mesh.labels.iter().map(|&l| {
                let k = l as usize;
                own.get(k).copied().flatten().or(*colour).unwrap_or([0.7, 0.7, 0.7])
            })),
        }
        all.merge(mesh);
    }
    match extension.as_str() {
        "obj" => {
            let mtl = std::path::Path::new(path).with_extension("mtl");
            let library = mtl
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let mut text = String::new();
            for (name, c, gloss) in &materials {
                let round = |v: f64| format!("{:.4}", v).trim_end_matches('0').trim_end_matches('.').to_string();
                text.push_str(&format!(
                    "newmtl {name}\nKd {} {} {}\n",
                    round(c[0]),
                    round(c[1]),
                    round(c[2])
                ));
                if *gloss > 0.0 {
                    text.push_str(&format!(
                        "Ks {g} {g} {g}\nNs {}\nPr {}\n",
                        round(10.0 + 990.0 * gloss),
                        round(1.0 - gloss),
                        g = round(*gloss)
                    ));
                }
                text.push('\n');
            }
            std::fs::write(&mtl, text)?;
            let out = format!("mtllib {library}\n{}", crate::mesh::write_obj_coloured(&all, &vertex_colours));
            std::fs::write(path, out)?
        }
        "stl" => {
            let mut out = vec![0u8; 80];
            out.extend((all.triangles.len() as u32).to_le_bytes());
            for &t in &all.triangles {
                let [a, b, c] = all.corners(t);
                let n = crate::mesh::normalized(crate::mesh::cross(
                    crate::mesh::sub(b, a),
                    crate::mesh::sub(c, a),
                ));
                for v in [n, a, b, c] {
                    for x in v {
                        out.extend((x as f32).to_le_bytes());
                    }
                }
                out.extend([0, 0]);
            }
            std::fs::write(path, out)?
        }
        "3mf" => std::fs::write(path, three_mf_meshes(&bodies))?,
        other => bail!("a file with mesh bodies exports as .obj, .stl or .3mf, not `.{other}`"),
    }
    Ok(())
}

pub fn export_model(model: &crate::model::Model, file: &std::path::Path, path: &str) -> Result<()> {
    if !model.meshes().is_empty() {
        return export_meshes(model, path);
    }
    let name = file
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let name = name.as_str();
    if let Some(spec) = model.sim.as_deref() {
        let parts: Vec<_> = spec
            .named()?
            .into_iter()
            .map(|(_, solid)| (solid, None))
            .collect();
        return export_coloured(&parts, path);
    }
    let step = path.to_ascii_lowercase();
    if model.assembly && step.ends_with(".svg") {
        let exploded = model.exploded_parts(if model.explode.is_empty() { 0.0 } else { 1.0 });
        let parts: Vec<(&Solid, Option<Colour>)> = exploded.iter().map(|(s, c)| (s, *c)).collect();
        let list = parts_list(model, file);
        std::fs::write(
            path,
            crate::drawing::annotated(&parts, None, Some(&list), &[]),
        )?;
        return Ok(());
    }
    if model.assembly && (step.ends_with(".step") || step.ends_with(".stp")) {
        let (products, instances) = assembly(model);
        std::fs::write(path, assembly_step(name, &products, &instances))?;
        return Ok(());
    }
    if step.ends_with(".dxf") {
        let sheet = model
            .sheet
            .as_ref()
            .filter(|sheet| !sheet.plates.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("a .dxf export is the flat pattern of a sheet metal part; start the part with `sheet` and `tab`")
            })?;
        std::fs::write(path, crate::model::flat_dxf(sheet, model.solid.as_ref()))?;
        return Ok(());
    }
    if step.ends_with(".svg") {
        let svg = crate::drawing::drawing_noted(&model.parts(), None, &model.hole_notes);
        std::fs::write(path, svg)?;
        return Ok(());
    }
    if step.ends_with(".3mf") {
        std::fs::write(path, three_mf(&model.named_parts()))?;
        return Ok(());
    }
    export_coloured(&model.parts(), path)
}

fn quoted(text: &str) -> String {
    text.replace('\'', "''")
}

pub fn assembly_step(name: &str, products: &[Product], instances: &[Instance<'_>]) -> String {
    let parts: Vec<(&Solid, Option<Colour>)> =
        products.iter().map(|p| (&p.solid, p.colour)).collect();
    let step = step_text(&parts);
    let breps = solid_reps(&step);
    let find = |kind: &str| {
        step.lines()
            .find(|line| line.contains(kind))
            .and_then(entity_id)
    };
    let (Some(root_rep), Some(root_pd), Some(root_product), Some(root_context)) = (
        find("= ADVANCED_BREP_SHAPE_REPRESENTATION("),
        find("= PRODUCT_DEFINITION('design'"),
        find("= PRODUCT('"),
        find("= PRODUCT_CONTEXT("),
    ) else {
        return step;
    };
    let definition_context = find("= PRODUCT_DEFINITION_CONTEXT(").unwrap_or(0);
    let geometry_context = step
        .lines()
        .find(|line| line.contains("= ADVANCED_BREP_SHAPE_REPRESENTATION("))
        .and_then(|line| line.rsplit('#').next())
        .and_then(|tail| tail.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse::<usize>().ok())
        .unwrap_or(0);
    if breps.len() != products.len() {
        return step;
    }
    let mut ids = Ids(step.lines().filter_map(entity_id).max().unwrap_or(0) + 1);
    let mut added = String::new();
    let axis = |ids: &mut Ids, added: &mut String, m: Matrix4| {
        let (o, z, x) = (
            m.w.truncate(),
            m.z.truncate().normalize(),
            m.x.truncate().normalize(),
        );
        let (p, dz, dx, a) = (ids.take(), ids.take(), ids.take(), ids.take());
        let _ = writeln!(
            added,
            "#{p} = CARTESIAN_POINT('', ({:.9}, {:.9}, {:.9}));\n\
             #{dz} = DIRECTION('', ({:.12}, {:.12}, {:.12}));\n\
             #{dx} = DIRECTION('', ({:.12}, {:.12}, {:.12}));\n\
             #{a} = AXIS2_PLACEMENT_3D('', #{p}, #{dz}, #{dx});",
            o.x, o.y, o.z, z.x, z.y, z.z, x.x, x.y, x.z
        );
        a
    };
    let root_axis = axis(&mut ids, &mut added, Matrix4::identity());
    let mut product_reps = Vec::new();
    for (product, brep) in products.iter().zip(&breps) {
        let label = quoted(&product.name);
        let own_axis = axis(&mut ids, &mut added, Matrix4::identity());
        let [prod, pdf, pd, pds, rep, sdr] = ids.many();
        let _ = writeln!(
            added,
            "#{prod} = PRODUCT('{label}', '{label}', '', (#{root_context}));\n\
             #{pdf} = PRODUCT_DEFINITION_FORMATION('', '', #{prod});\n\
             #{pd} = PRODUCT_DEFINITION('design', '', #{pdf}, #{definition_context});\n\
             #{pds} = PRODUCT_DEFINITION_SHAPE('', '', #{pd});\n\
             #{rep} = ADVANCED_BREP_SHAPE_REPRESENTATION('{label}', (#{brep}, #{own_axis}), #{geometry_context});\n\
             #{sdr} = SHAPE_DEFINITION_REPRESENTATION(#{pds}, #{rep});"
        );
        product_reps.push((pd, rep, own_axis));
    }
    let mut placed_axes = vec![root_axis];
    for (k, instance) in instances.iter().enumerate() {
        let (pd, rep, own_axis) = product_reps[instance.product];
        let there = axis(&mut ids, &mut added, instance.placement);
        placed_axes.push(there);
        let label = quoted(&instance.name);
        let [transform, relation, nauo, pds, cdsr] = ids.many();
        let _ = writeln!(
            added,
            "#{transform} = ITEM_DEFINED_TRANSFORMATION('', '', #{own_axis}, #{there});\n\
             #{relation} = ( REPRESENTATION_RELATIONSHIP('', '', #{rep}, #{root_rep}) REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#{transform}) SHAPE_REPRESENTATION_RELATIONSHIP() );\n\
             #{nauo} = NEXT_ASSEMBLY_USAGE_OCCURRENCE('{}', '{label}', '', #{root_pd}, #{pd}, $);\n\
             #{pds} = PRODUCT_DEFINITION_SHAPE('{label}', '', #{nauo});\n\
             #{cdsr} = CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(#{relation}, #{pds});",
            k + 1
        );
    }
    let assembly_name = quoted(name);
    let root_items = placed_axes
        .iter()
        .map(|a| format!("#{a}"))
        .collect::<Vec<_>>()
        .join(", ");
    let rewritten: Vec<String> = step
        .lines()
        .map(|line| match entity_id(line) {
            Some(id) if id == root_rep => format!(
                "#{id} = SHAPE_REPRESENTATION('{assembly_name}', ({root_items}), #{geometry_context});"
            ),
            Some(id) if id == root_product => format!(
                "#{id} = PRODUCT('{assembly_name}', '{assembly_name}', '', (#{root_context}));"
            ),
            _ => line.to_string(),
        })
        .collect();
    let step = rewritten.join("\n") + "\n";
    match step.rfind("ENDSEC;") {
        Some(at) => format!("{}{added}{}", &step[..at], &step[at..]),
        None => step,
    }
}

struct Ids(usize);

impl Ids {
    fn take(&mut self) -> usize {
        self.0 += 1;
        self.0 - 1
    }

    fn many<const N: usize>(&mut self) -> [usize; N] {
        std::array::from_fn(|_| self.take())
    }
}

const SMOOTH_LEADER_SPANS: usize = 32;

fn smooth_through(points: &[Point3], (t0, t1): (f64, f64)) -> Option<BsplineCurve<Point3>> {
    let count = points.len();
    if count < 4 {
        return None;
    }
    let lengths: Vec<f64> = points.windows(2).map(|w| w[0].distance(w[1])).collect();
    let total: f64 = lengths.iter().sum();
    let parameters: Vec<f64> = std::iter::once(0.0)
        .chain(lengths.iter().scan(0.0, |run, length| {
            *run += length / total;
            Some(*run)
        }))
        .map(|u| t0 + (t1 - t0) * u)
        .collect();
    let knots: Vec<f64> = std::iter::repeat_n(t0, 4)
        .chain((1..count - 3).map(|j| parameters[j..j + 3].iter().sum::<f64>() / 3.0))
        .chain(std::iter::repeat_n(t1, 4))
        .collect();
    let pairs: Vec<(f64, Point3)> = parameters.into_iter().zip(points.iter().copied()).collect();
    BsplineCurve::try_interpolate(KnotVector::from(knots), pairs).ok()
}

fn even_leader(curve: &Curve) -> Curve {
    let Curve::IntersectionCurve(intersection) = curve else {
        return curve.clone();
    };
    let Curve::BsplineCurve(leader) = intersection.leader().as_ref() else {
        return curve.clone();
    };
    let poles = leader.control_points();
    if leader.degree() != 1 || poles.len() < 4 {
        return curve.clone();
    }
    let lengths: Vec<f64> = std::iter::once(0.0)
        .chain(poles.windows(2).scan(0.0, |total, pair| {
            *total += pair[0].distance(pair[1]);
            Some(*total)
        }))
        .collect();
    let total = *lengths.last().unwrap_or(&0.0);
    if total <= 0.0 {
        return curve.clone();
    }
    let (t0, t1) = leader.range_tuple();
    let knots = leader.knot_vector();
    let pole_parameter = |i: usize| knots[i + 1];
    let last = poles.len() - 1;
    let count = last.min(SMOOTH_LEADER_SPANS);
    let points: Vec<Point3> = (0..=count)
        .map(|k| {
            if k == 0 || k == count {
                return poles[if k == 0 { 0 } else { last }];
            }
            let want = total * k as f64 / count as f64;
            let i = lengths.partition_point(|&l| l <= want).clamp(1, last) - 1;
            let span = lengths[i + 1] - lengths[i];
            let f = if span > 0.0 {
                (want - lengths[i]) / span
            } else {
                0.0
            };
            let t = pole_parameter(i) + (pole_parameter(i + 1) - pole_parameter(i)) * f;
            curve.subs(t)
        })
        .collect();
    let mut even = intersection.clone();
    **even.leader_mut() =
        Curve::BsplineCurve(smooth_through(&points, (t0, t1)).unwrap_or_else(|| {
            let mut knots = KnotVector::uniform_knot(1, count);
            knots.transform(t1 - t0, t0);
            BsplineCurve::new(knots, points.clone())
        }));
    Curve::IntersectionCurve(even)
}

fn even_leaders(solid: &Solid) -> Solid {
    solid.mapped(|p| *p, even_leader, |s| s.clone())
}

fn entity_id(line: &str) -> Option<usize> {
    line.trim_start()
        .strip_prefix('#')?
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

fn solid_reps(step: &str) -> Vec<usize> {
    step.lines()
        .filter(|line| {
            line.contains("= MANIFOLD_SOLID_BREP(") || line.contains("= BREP_WITH_VOIDS(")
        })
        .filter_map(entity_id)
        .collect()
}

fn with_colours(step: &str, parts: &[(&Solid, Option<Colour>)]) -> String {
    let breps = solid_reps(step);
    let context = step
        .lines()
        .find(|line| line.contains("= ADVANCED_BREP_SHAPE_REPRESENTATION("))
        .and_then(|line| line.rsplit('#').next())
        .and_then(|tail| tail.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse::<usize>().ok());
    let (Some(context), true) = (context, breps.len() == parts.len()) else {
        return step.to_string();
    };
    let mut next = step.lines().filter_map(entity_id).max().unwrap_or(0) + 1;
    let mut added = String::new();
    let mut styled = Vec::new();
    for (brep, (_, colour)) in breps.iter().zip(parts) {
        let Some([r, g, b]) = colour else { continue };
        let id = next;
        let _ = writeln!(
            added,
            "#{} = COLOUR_RGB('', {r:.4}, {g:.4}, {b:.4});\n\
             #{} = FILL_AREA_STYLE_COLOUR('', #{});\n\
             #{} = FILL_AREA_STYLE('', (#{}));\n\
             #{} = SURFACE_STYLE_FILL_AREA(#{});\n\
             #{} = SURFACE_SIDE_STYLE('', (#{}));\n\
             #{} = SURFACE_STYLE_USAGE(.BOTH., #{});\n\
             #{} = PRESENTATION_STYLE_ASSIGNMENT((#{}));\n\
             #{} = STYLED_ITEM('color', (#{}), #{brep});",
            id,
            id + 1,
            id,
            id + 2,
            id + 1,
            id + 3,
            id + 2,
            id + 4,
            id + 3,
            id + 5,
            id + 4,
            id + 6,
            id + 5,
            id + 7,
            id + 6,
        );
        styled.push(format!("#{}", id + 7));
        next += 8;
    }
    let _ = writeln!(
        added,
        "#{next} = MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION('', ({}), #{context});",
        styled.join(", ")
    );
    match step.rfind("ENDSEC;") {
        Some(at) => format!("{}{added}{}", &step[..at], &step[at..]),
        None => step.to_string(),
    }
}

fn three_mf(parts: &[(String, &Solid, Option<Colour>)]) -> Vec<u8> {
    let meshes: Vec<(String, crate::mesh::Mesh, Option<Colour>)> = parts
        .iter()
        .map(|(name, solid, colour)| (name.clone(), as_mesh(solid, name), *colour))
        .collect();
    three_mf_meshes(&meshes)
}

fn three_mf_meshes(parts: &[(String, crate::mesh::Mesh, Option<Colour>)]) -> Vec<u8> {
    let mut model = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n<resources>\n",
    );
    let coloured = parts.iter().any(|(_, _, c)| c.is_some());
    if coloured {
        model.push_str("<basematerials id=\"1\">\n");
        for (name, _, colour) in parts {
            let [r, g, b] = colour
                .unwrap_or([0.7, 0.7, 0.7])
                .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            let _ = writeln!(
                model,
                "<base name=\"{}\" displaycolor=\"#{r:02X}{g:02X}{b:02X}\"/>",
                name.replace('"', "&quot;")
            );
        }
        model.push_str("</basematerials>\n");
    }
    for (i, (name, mesh, _)) in parts.iter().enumerate() {
        let (points, triangles) = (&mesh.positions, &mesh.triangles);
        let material = if coloured {
            format!(" pid=\"1\" pindex=\"{i}\"")
        } else {
            String::new()
        };
        let _ = writeln!(
            model,
            "<object id=\"{}\" name=\"{}\" type=\"model\"{material}>\n<mesh>\n<vertices>",
            i + 2,
            name.replace('"', "&quot;")
        );
        for p in points {
            let _ = writeln!(
                model,
                "<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>",
                p[0], p[1], p[2]
            );
        }
        model.push_str("</vertices>\n<triangles>\n");
        for [a, b, c] in triangles {
            let _ = writeln!(model, "<triangle v1=\"{a}\" v2=\"{b}\" v3=\"{c}\"/>");
        }
        model.push_str("</triangles>\n</mesh>\n</object>\n");
    }
    model.push_str("</resources>\n<build>\n");
    for i in 0..parts.len() {
        let _ = writeln!(model, "<item objectid=\"{}\"/>", i + 2);
    }
    model.push_str("</build>\n</model>\n");
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/></Types>\n";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/></Relationships>\n";
    stored_zip(&[
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("3D/3dmodel.model", model.as_bytes()),
    ])
}

fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |crc, &byte| {
        (0..8).fold(crc ^ byte as u32, |c, _| {
            if c & 1 == 1 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            }
        })
    })
}

fn stored_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let (crc, size, name_len) = (crc32(data), data.len() as u32, name.len() as u16);
        let common = |buffer: &mut Vec<u8>| {
            buffer.extend_from_slice(&20u16.to_le_bytes());
            buffer.extend_from_slice(&0u16.to_le_bytes());
            buffer.extend_from_slice(&0u16.to_le_bytes());
            buffer.extend_from_slice(&0u16.to_le_bytes());
            buffer.extend_from_slice(&0x21u16.to_le_bytes());
            buffer.extend_from_slice(&crc.to_le_bytes());
            buffer.extend_from_slice(&size.to_le_bytes());
            buffer.extend_from_slice(&size.to_le_bytes());
            buffer.extend_from_slice(&name_len.to_le_bytes());
            buffer.extend_from_slice(&0u16.to_le_bytes());
        };
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        common(&mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        common(&mut central);
        central.extend_from_slice(&[0u8; 10]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

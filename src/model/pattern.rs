use super::Model;
use super::args::{Args, point3};
use super::assembly::{Condition, Feature};
use crate::parse::{Line, parse_line};
use anyhow::{Result, anyhow, bail};
use monstertruck::modeling::*;

impl Model {
    fn copy_part(&mut self, part: &str, name: &str, transform: Matrix4) -> Result<()> {
        if self.body_names().iter().any(|n| n == name) {
            bail!("there is already a part called `{name}`");
        }
        let solid = self.named_body(part)?;
        let moved = super::solids::placed(&solid, transform);
        self.bodies.push((name.to_string(), moved));
        fn copy<T: Clone>(map: &mut std::collections::HashMap<String, T>, from: &str, to: &str) {
            if let Some(value) = map.get(from).cloned() {
                map.insert(to.to_string(), value);
            }
        }
        copy(&mut self.colours, part, name);
        copy(&mut self.opacities, part, name);
        copy(&mut self.materials, part, name);
        copy(&mut self.sources, part, name);
        if self.members.iter().any(|member| member == part) {
            self.members.push(name.to_string());
        }
        let exploded: Vec<(String, Vector3)> = self
            .explode
            .iter()
            .filter(|(owner, _)| owner == part)
            .map(|(_, by)| (name.to_string(), transform.transform_vector(*by)))
            .collect();
        self.explode.extend(exploded);
        let placed = self
            .placements
            .get(part)
            .copied()
            .unwrap_or_else(Matrix4::identity);
        self.placements.insert(name.to_string(), transform * placed);
        // Patterns copy an evaluated body, not its containing subassembly.
        self.assembly_instances
            .push(super::assembly::AssemblyInstance {
                name: name.to_string(),
                placement: transform * placed,
                bodies: vec![name.to_string()],
                children: Vec::new(),
            });
        if let Some(groups) = self.part_groups.get(part) {
            let moved = groups.moved(transform);
            self.part_groups.insert(name.to_string(), moved);
        }
        Ok(())
    }

    fn replay_mates(&mut self, part: &str, name: &str, transform: Matrix4) -> Result<usize> {
        let mates: Vec<(String, Feature)> = self
            .mates
            .iter()
            .filter(|m| m.parts[0] == part)
            .map(|m| (m.text.clone(), m.features[1]))
            .collect();
        let mut done = 0;
        let mut seen = Vec::new();
        for (text, target) in mates {
            if seen.contains(&text) {
                continue;
            }
            seen.push(text.clone());
            let Ok(mut line) = parse_line(0, &text) else {
                continue;
            };
            let renamed = |word: &str| match word.split_once(':') {
                Some((owner, faces)) if owner == part => format!("{name}:{faces}"),
                _ => word.to_string(),
            };
            line.positional = line.positional.iter().map(|w| renamed(w)).collect();
            if line.op == "concentric" {
                let at = transform.transform_point(target.point());
                line.named.retain(|(key, _)| key != "near");
                line.named
                    .push(("near".to_string(), format!("{},{},{}", at.x, at.y, at.z)));
            }
            line.text = std::iter::once(line.op.clone())
                .chain(line.positional.iter().cloned())
                .chain(line.named.iter().map(|(k, v)| format!("{k}={v}")))
                .collect::<Vec<_>>()
                .join(" ");
            self.apply_assembly(&line)
                .map_err(|e| anyhow!("copying `{text}` to `{name}`: {e:#}"))?;
            done += 1;
        }
        Ok(done)
    }

    fn replay_joints(&mut self, part: &str, name: &str, transform: Matrix4) -> Result<usize> {
        let joints: Vec<super::assembly::Joint> = self
            .joints
            .iter()
            .filter(|j| j.child == part && j.movable())
            .cloned()
            .collect();
        for joint in &joints {
            let copy = format!("{}{}", joint.name, &name[part.len()..]);
            if self.joints.iter().any(|j| j.name == copy) {
                bail!("there is already a joint called `{copy}`");
            }
            self.joints.push(super::assembly::Joint {
                name: copy.clone(),
                child: name.to_string(),
                ..joint.moved(transform)
            });
            let couples: Vec<super::assembly::Couple> = self
                .couples
                .iter()
                .filter(|c| c.driven == joint.name)
                .cloned()
                .collect();
            self.couples
                .extend(couples.into_iter().map(|c| super::assembly::Couple {
                    driven: copy.clone(),
                    ..c
                }));
        }
        Ok(joints.len())
    }

    pub(crate) fn op_pattern(&mut self, line: &Line) -> Result<String> {
        let args = Args::new(
            line,
            &["part"],
            &["holes", "count", "step", "angle", "axis"],
            false,
        )?;
        let part = args.text("part")?.to_string();
        if !self.body_names().contains(&part) {
            bail!("no part called `{part}`; parts are {:?}", self.body_names());
        }
        let moves: Vec<Matrix4> = match args.values.get("holes") {
            Some(selector) => {
                if args.has("count") || args.has("step") || args.has("angle") {
                    bail!("give `holes=`, or `count=` with `step=` or `angle=`, not both");
                }
                let (_, holes) = self.holes(selector)?;
                let (owner, _) = selector
                    .split_once(':')
                    .ok_or_else(|| anyhow!("write `holes=part:faces`"))?;
                let base = self
                    .mates
                    .iter()
                    .filter(|m| m.parts[0] == part && m.parts[1] == owner)
                    .filter(|m| m.condition == Condition::Concentric)
                    .find_map(|m| match m.features[1] {
                        Feature::Axis(p, d) => holes
                            .iter()
                            .find(|h| {
                                let gap = h.point - p;
                                h.axis.cross(d).magnitude() < 1.0e-6
                                    && (gap - d * gap.dot(d)).magnitude() < 1.0e-3
                            })
                            .copied(),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        anyhow!("`{part}` is not concentric with one of `{selector}`; mate it into one first")
                    })?;
                holes
                    .iter()
                    .filter(|h| {
                        let gap = h.point - base.point;
                        (gap - base.axis * gap.dot(base.axis)).magnitude() > 1.0e-3
                    })
                    .map(|h| {
                        if h.axis.cross(base.axis).magnitude() > 1.0e-6 {
                            bail!("the holes in `{selector}` are not parallel");
                        }
                        let gap = h.point - base.point;
                        Ok(Matrix4::from_translation(
                            gap - base.axis * gap.dot(base.axis),
                        ))
                    })
                    .collect::<Result<_>>()?
            }
            None => {
                let count = args.number("count", &self.scope)?;
                if !count.is_finite() || count < 1.0 || count.fract() != 0.0 {
                    bail!("count must be a positive whole number");
                }
                let each = match (args.values.get("step"), args.values.get("angle")) {
                    (Some(step), None) => {
                        Matrix4::from_translation(point3(step, &self.scope)?.to_vec())
                    }
                    (None, Some(_)) => {
                        let angle = args.number("angle", &self.scope)? / count;
                        let name = args.values.get("axis").ok_or_else(|| {
                            anyhow!("a turning pattern needs `axis=` a datum axis")
                        })?;
                        let (through, axis) = self
                            .axes
                            .get(*name)
                            .copied()
                            .ok_or_else(|| anyhow!("no datum axis `{name}`"))?;
                        Matrix4::from_translation(through.to_vec())
                            * Matrix4::from_axis_angle(axis, Deg(angle))
                            * Matrix4::from_translation(-through.to_vec())
                    }
                    _ => bail!("give `step=x,y,z` or `angle=` with `axis=`"),
                };
                (1..count as usize)
                    .scan(Matrix4::identity(), |at, _| {
                        *at = each * *at;
                        Some(*at)
                    })
                    .collect()
            }
        };
        let mut made = Vec::new();
        let (mut mated, mut jointed) = (0, 0);
        for (k, transform) in moves.iter().enumerate() {
            let name = format!("{part}_{}", k + 2);
            self.copy_part(&part, &name, *transform)?;
            mated += self.replay_mates(&part, &name, *transform)?;
            jointed += self.replay_joints(&part, &name, *transform)?;
            made.push(name);
        }
        let each = |n: usize, what: &str| match n {
            0 => String::new(),
            n => format!(", each with {} {what} of `{part}`", n / made.len().max(1)),
        };
        let summary = if made.is_empty() || made.len() > 12 {
            format!("{} instances of `{part}` (including seed)", made.len() + 1)
        } else {
            made.join(", ")
        };
        Ok(format!(
            "{}{}{}",
            summary,
            each(mated, "mate(s)"),
            each(jointed, "joint(s)")
        ))
    }
}

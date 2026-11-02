//! Preserve evaluator instance ancestry while retaining established body identities.
use super::scene::{Node, Scene, Transform};
use anyhow::{Context, Result, ensure};
use monstertruck::modeling::{Matrix4, SquareMatrix};
use std::collections::{HashMap, HashSet};

pub(crate) type BodyParents = HashMap<String, (usize, Matrix4)>;

pub(crate) fn append(
    scene: &mut Scene,
    owner: usize,
    instances: &[crate::model::AssemblyInstance],
    selected: &HashSet<String>,
) -> Result<BodyParents> {
    fn visit(
        scene: &mut Scene,
        instance: &crate::model::AssemblyInstance,
        parent: usize,
        parent_frame: Matrix4,
        prefix: &str,
        selected: &HashSet<String>,
        output: &mut BodyParents,
    ) -> Result<()> {
        let members: Vec<_> = instance
            .bodies
            .iter()
            .filter(|b| selected.contains(*b))
            .collect();
        if members.is_empty() {
            return Ok(());
        }
        // A terminal single-body part is already represented by its mesh node.
        // Preserve explicit assembly ancestors even when selection leaves one body.
        let (parent, frame, prefix) = if !instance.children.is_empty() || instance.bodies.len() > 1
        {
            let name = format!("{prefix}{}/", instance.name);
            ensure!(
                !scene.nodes.iter().any(|n| n.name == name),
                "assembly group node name collision: {name}"
            );
            let index = scene.nodes.len();
            scene.nodes.push(Node {
                name: name.clone(),
                parent: Some(parent),
                mesh: None,
                transform: Transform::from_matrix(
                    parent_frame
                        .invert()
                        .context("singular assembly parent frame")?
                        * instance.placement,
                )?,
            });
            (index, instance.placement, name)
        } else {
            (parent, parent_frame, prefix.to_string())
        };
        for body in members {
            output.insert(body.clone(), (parent, frame));
        }
        for child in &instance.children {
            visit(scene, child, parent, frame, &prefix, selected, output)?;
        }
        Ok(())
    }
    let mut output = HashMap::new();
    let prefix = format!("{}/", scene.nodes[owner].name);
    for instance in instances {
        visit(
            scene,
            instance,
            owner,
            Matrix4::identity(),
            &prefix,
            selected,
            &mut output,
        )?;
    }
    Ok(output)
}

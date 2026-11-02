//! Digest-bound consumer roles for managed FBX packages. Generic FBX geometry
//! remains intact; the matching engine adapter explicitly applies the roles.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(super) fn create(glb: &[u8], digest: &str) -> Result<Option<Vec<u8>>> {
    ensure!(
        glb.len() >= 28 && &glb[..4] == b"glTF",
        "consumer receipt requires GLB"
    );
    let length = u32::from_le_bytes(glb[12..16].try_into()?) as usize;
    ensure!(
        length <= glb.len() - 20 && &glb[16..20] == b"JSON",
        "invalid GLB JSON chunk"
    );
    let source: Value = serde_json::from_slice(&glb[20..20 + length])?;
    let Some(value) = source["extras"].get("gcad_consumer_profile") else {
        return Ok(None);
    };
    let profile: super::consumer::Profile = serde_json::from_value(value.clone())?;
    let raw = source["nodes"]
        .as_array()
        .context("consumer nodes required")?;
    ensure!(
        !raw.is_empty() && raw.len() <= 65_536,
        "consumer node count exceeds bound"
    );
    let mut nodes = Vec::with_capacity(raw.len());
    for node in raw {
        nodes.push(super::scene::Node {
            name: node["name"]
                .as_str()
                .context("consumer named node required")?
                .to_owned(),
            mesh: node
                .get("mesh")
                .map(|v| v.as_u64().context("invalid mesh index").map(|n| n as usize))
                .transpose()?,
            ..Default::default()
        });
    }
    for (i, node) in raw.iter().enumerate() {
        if let Some(children) = node.get("children") {
            for child in children.as_array().context("invalid child array")? {
                let child = child.as_u64().context("invalid child index")? as usize;
                let target = nodes.get_mut(child).context("child outside nodes")?;
                ensure!(target.parent.replace(i).is_none(), "multiple node parents");
            }
        }
    }
    let graph = super::scene::Scene {
        nodes,
        // Only graph/mesh-index validation is needed here. Primitive counts are
        // read from the actual accessors below, never invented geometry.
        meshes: source["meshes"]
            .as_array()
            .context("consumer meshes required")?
            .iter()
            .enumerate()
            .map(|(i, _)| super::scene::SceneMesh {
                name: format!("mesh_{i}"),
                primitives: Vec::new(),
            })
            .collect(),
        ..Default::default()
    };
    graph.world_transforms()?;
    let scene = source["scene"]
        .as_u64()
        .context("active consumer scene required")? as usize;
    let roots = source["scenes"][scene]["nodes"]
        .as_array()
        .context("active consumer roots required")?;
    let mut active = vec![false; raw.len()];
    let mut children = vec![Vec::new(); raw.len()];
    for (i, node) in graph.nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            children[parent].push(i);
        }
    }
    let mut pending = Vec::new();
    for root in roots {
        let root = root.as_u64().context("invalid root")? as usize;
        ensure!(
            root < raw.len() && graph.nodes[root].parent.is_none(),
            "invalid active root"
        );
        pending.push(root);
    }
    while let Some(node) = pending.pop() {
        ensure!(!active[node], "duplicate active node");
        active[node] = true;
        pending.extend_from_slice(&children[node]);
    }
    let references = profile
        .sockets
        .iter()
        .map(|s| s.node.as_str())
        .chain(profile.colliders.iter().map(|c| c.node.as_str()))
        .chain(profile.lods.iter().flat_map(|g| {
            std::iter::once(g.root.as_str()).chain(
                g.levels
                    .iter()
                    .flat_map(|l| l.nodes.iter().map(String::as_str)),
            )
        }));
    for name in references {
        let node = graph
            .nodes
            .iter()
            .position(|n| n.name == name)
            .context("consumer node absent")?;
        ensure!(active[node], "consumer reference is outside active scene");
    }
    profile.validate_graph(&graph.nodes, |mesh| {
        let primitives = source["meshes"][mesh]["primitives"]
            .as_array()
            .context("missing collider mesh")?;
        let mut count = 0usize;
        for primitive in primitives {
            ensure!(
                primitive.get("mode").is_none_or(|m| m == 4),
                "triangle collider mesh required"
            );
            let index = primitive["indices"]
                .as_u64()
                .context("indexed collider mesh required")? as usize;
            let indices = source["accessors"][index]["count"]
                .as_u64()
                .context("invalid collider indices")? as usize;
            ensure!(
                indices > 0 && indices % 3 == 0,
                "invalid collider triangle count"
            );
            count = count
                .checked_add(indices / 3)
                .context("collider count overflow")?;
        }
        Ok(count)
    })?;
    for collider in &profile.colliders {
        let node = graph
            .nodes
            .iter()
            .position(|n| n.name == collider.node)
            .unwrap();
        ensure!(
            raw[node].get("skin").is_none(),
            "collision shapes cannot use skinned geometry"
        );
    }
    ensure!(
        digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid FBX digest"
    );
    let bytes = serde_json::to_vec_pretty(
        &json!({"version":1,"format":"gcad-consumer-profile","model_sha256":digest,
        "source_glb_sha256":format!("{:x}",Sha256::digest(glb)),"profile":profile}),
    )?;
    ensure!(
        bytes.len() <= 4 * 1024 * 1024,
        "consumer receipt exceeds 4 MiB"
    );
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_consumer_receipt_binds_digest_and_preserves_roles() {
        let asset = crate::asset::author::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/game-assets/plate-consumer-profile.gasset"),
            &[],
        )
        .unwrap();
        let glb = asset.encode_glb().unwrap();
        let receipt: Value =
            serde_json::from_slice(&create(&glb, &"ab".repeat(32)).unwrap().unwrap()).unwrap();
        assert_eq!(
            receipt["profile"],
            serde_json::to_value(asset.scene.consumer.unwrap()).unwrap()
        );
        assert_eq!(receipt["model_sha256"], "ab".repeat(32));
        assert_eq!(
            receipt["source_glb_sha256"],
            format!("{:x}", Sha256::digest(glb))
        );
    }
}

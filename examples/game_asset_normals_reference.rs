//! Full-precision geometry-only reference for independent crease-normal checks.
//! No expected normal values or smoothing groups are supplied to the verifier.
use anyhow::{Context, Result, ensure};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 3,
        "usage: game_asset_normals_reference INPUT.gasset NEW_REFERENCE.json"
    );
    let asset = gcad::asset::author::load(std::path::Path::new(&args[1]), &[])?;
    let node = asset
        .scene
        .nodes
        .iter()
        .find(|n| n.name == "hard")
        .context("fixture needs a hard node")?;
    let mesh = &asset.scene.meshes[node.mesh.context("hard node has no mesh")?];
    ensure!(mesh.primitives.len() == 1, "expected one primitive");
    let geometry = &mesh.primitives[0].geometry;
    let reference = serde_json::json!({"version":1,"units":"mm","coordinates":"right-handed-z-up",
        "positions":geometry.positions,"triangles":geometry.triangles});
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    serde_json::to_writer_pretty(file, &reference)?;
    Ok(())
}

//! Native xatlas unwrapping. Geometry is copied through exact source indices;
//! only attribute tuples and UVs change. Existing skin bindings must be rebound.
use super::Mesh;
use anyhow::{Result, ensure};

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub resolution: u32,
    pub padding: u32,
}
impl Settings {
    pub fn validate(self) -> Result<()> {
        ensure!(
            (64..=4096).contains(&self.resolution) && self.resolution.is_power_of_two(),
            "unwrap resolution must be a power of two in 64..4096"
        );
        ensure!(
            self.padding >= 2 && self.padding <= self.resolution / 16,
            "unwrap padding must be 2..resolution/16 pixels"
        );
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
pub fn mesh(_source: &Mesh, _settings: Settings) -> Result<Mesh> {
    anyhow::bail!("automatic UV unwrapping requires a native build")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn mesh(source: &Mesh, settings: Settings) -> Result<Mesh> {
    use xatlas_rs_v2::{ChartOptions, IndexData, MeshData, MeshDecl, PackOptions, Xatlas};
    settings.validate()?;
    source.validate()?;
    ensure!(
        source.positions.len() <= 750_000,
        "unwrap vertex budget exceeded"
    );
    ensure!(
        !source.triangles.is_empty() && source.triangles.len() <= 250_000,
        "unwrap requires 1..250000 triangles per mesh"
    );
    // Normalize native input for float32 precision; copy original f64 tuples back.
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for p in &source.positions {
        for k in 0..3 {
            low[k] = low[k].min(p[k]);
            high[k] = high[k].max(p[k]);
        }
    }
    let extent = (0..3).map(|k| high[k] - low[k]).fold(0f64, f64::max);
    ensure!(extent.is_finite() && extent > 0., "invalid unwrap bounds");
    let positions: Vec<f32> = source
        .positions
        .iter()
        .flat_map(|p| (0..3).map(move |k| ((p[k] - low[k]) / extent) as f32))
        .collect();
    let normals: Vec<f32> = source.normals.iter().flatten().map(|v| *v as f32).collect();
    let indices: Vec<u32> = source.triangles.iter().flatten().copied().collect();
    // Reject triangles that collapse during the float32 conversion, before FFI.
    for t in &source.triangles {
        let p = t.map(|i| &positions[i as usize * 3..i as usize * 3 + 3]);
        let a: [f32; 3] = std::array::from_fn(|k| p[1][k] - p[0][k]);
        let b: [f32; 3] = std::array::from_fn(|k| p[2][k] - p[0][k]);
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        ensure!(
            cross.iter().any(|v| *v != 0.),
            "triangle collapses at unwrap precision"
        );
    }
    let decl = MeshDecl {
        vertex_position_data: MeshData::Contiguous(&positions),
        vertex_normal_data: Some(MeshData::Contiguous(&normals)),
        index_data: Some(IndexData::U32(&indices)),
        ..Default::default()
    };
    let mut atlas = Xatlas::new();
    atlas
        .add_mesh(&decl)
        .map_err(|e| anyhow::anyhow!("xatlas input: {e:?}"))?;
    atlas.generate(
        &ChartOptions {
            normal_seam_weight: 1001.,
            ..Default::default()
        },
        &PackOptions {
            resolution: settings.resolution,
            padding: settings.padding,
            rotate_charts: false,
            rotate_charts_to_axis: false,
            ..Default::default()
        },
    );
    let mut density = atlas.texels_per_unit();
    for _ in 0..24 {
        if atlas.atlas_count() == 1
            && atlas.width() <= settings.resolution
            && atlas.height() <= settings.resolution
        {
            break;
        }
        density *= 0.85;
        atlas.pack_charts(&PackOptions {
            resolution: settings.resolution,
            padding: settings.padding,
            texels_per_unit: density,
            rotate_charts: false,
            rotate_charts_to_axis: false,
            ..Default::default()
        });
    }
    ensure!(
        atlas.atlas_count() == 1 && atlas.width() > 0 && atlas.height() > 0,
        "unwrap must produce one atlas; simplify the mesh or increase resolution"
    );
    // xatlas resolution is approximate. Preserve padding in an exact square image.
    ensure!(
        atlas.width() <= settings.resolution && atlas.height() <= settings.resolution,
        "unwrap packing exceeds resolution; increase resolution or reduce padding"
    );
    let meshes = atlas.meshes();
    let output = &meshes[0];
    ensure!(
        output.index_array.len() == indices.len(),
        "unwrap changed triangle count"
    );
    let mut mesh = Mesh {
        positions: Vec::new(),
        normals: Vec::new(),
        texcoords: Some(Vec::new()),
        tangents: None,
        triangles: Vec::new(),
    };
    for v in &output.vertex_array {
        ensure!(
            v.atlas_index == 0 && v.chart_index >= 0,
            "unwrap contains an unassigned face"
        );
        let i = v.xref as usize;
        ensure!(
            i < source.positions.len(),
            "unwrap returned invalid vertex correspondence"
        );
        mesh.positions.push(source.positions[i]);
        mesh.normals.push(source.normals[i]);
        mesh.texcoords
            .as_mut()
            .unwrap()
            .push(v.uv.map(|v| v as f64 / settings.resolution as f64));
    }
    for (old, new) in source
        .triangles
        .iter()
        .zip(output.index_array.chunks_exact(3))
    {
        let triangle: [u32; 3] = new.try_into().unwrap();
        ensure!(
            triangle
                .iter()
                .all(|i| (*i as usize) < output.vertex_array.len()),
            "invalid unwrap index"
        );
        ensure!(
            triangle.map(|i| output.vertex_array[i as usize].xref) == *old,
            "unwrap changed triangle order; cannot retain source-face lineage"
        );
        mesh.triangles.push(triangle);
    }
    mesh.generate_tangents()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn unwrap_preserves_corners_and_is_repeatable_with_invalid_settings_rejected() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/game-assets/plate-unwrapped.gasset");
        let a = super::super::author::load(&path, &[]).unwrap();
        let b = super::super::author::load_with_faces(&path, &[]).unwrap();
        b.face_map(0).unwrap();
        b.encode_glb().unwrap();
        let original =
            super::super::author::load(&path.with_file_name("plate.gasset"), &[]).unwrap();
        let a = &a.scene.meshes[0].primitives[0].geometry;
        let b = &b.scene.meshes[0].primitives[0].geometry;
        let original = &original.scene.meshes[0].primitives[0].geometry;
        assert_eq!(a.triangles.len(), original.triangles.len());
        for (t, original_t) in a.triangles.iter().zip(&original.triangles) {
            assert_eq!(
                t.map(|i| a.positions[i as usize]),
                original_t.map(|i| original.positions[i as usize])
            );
        }
        assert_eq!(a.positions, b.positions);
        assert_eq!(a.triangles, b.triangles);
        assert_eq!(a.texcoords, b.texcoords);
        assert!(
            a.texcoords
                .as_ref()
                .unwrap()
                .iter()
                .flatten()
                .all(|v| (0.0..=1.0).contains(v))
        );
        assert!(
            mesh(
                a,
                Settings {
                    resolution: 0,
                    padding: 8
                }
            )
            .is_err()
        );
        let mut bad = a.clone();
        bad.triangles[0] = [0, 0, 0];
        assert!(
            mesh(
                &bad,
                Settings {
                    resolution: 128,
                    padding: 8
                }
            )
            .is_err()
        );
        let source = std::fs::read_to_string(path).unwrap();
        assert!(
            super::super::author::Document::parse(&source.replace("padding=8", "padding=0"))
                .is_err()
        );
        assert!(
            super::super::author::Document::parse(
                &(source
                    + "\natlas plate width=1024 height=1024 padding=8 density=1 filter=linear\n")
            )
            .is_err()
        );
    }
}

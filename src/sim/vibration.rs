use super::fem::modal::solve;
use super::statics::prepare;
use super::{Field, Outcome, Reading, Spec};
use anyhow::Result;

pub fn run(spec: &Spec) -> Result<Outcome> {
    super::statics::stiff(spec)?;
    let prepared = prepare(spec)?;
    if !spec.supports.is_empty() {
        prepared.held(
            &spec
                .supports
                .iter()
                .flat_map(|s| s.faces.iter().copied())
                .collect::<Vec<_>>(),
            "support",
        )?;
    }
    let mesh = &prepared.mesh;
    let materials = prepared.elastic()?;
    let densities: Vec<f64> = prepared.parts.iter().map(|p| p.density * 1.0e-9).collect();
    let wanted = spec.modes.unwrap_or(6);
    let found = solve(
        mesh,
        &materials,
        &densities,
        &spec.supports,
        wanted,
        super::fem::gpu(),
    )?;
    let readings: Vec<Reading> = found
        .modes
        .iter()
        .enumerate()
        .map(|(k, mode)| Reading {
            name: format!("mode{}", k + 1),
            value: mode.frequency,
            unit: "Hz".into(),
            at: format!(
                "effective mass x {:.0}%, y {:.0}%, z {:.0}%",
                mode.effective[0] * 100.0,
                mode.effective[1] * 100.0,
                mode.effective[2] * 100.0
            ),
        })
        .collect();
    let mut notes = vec![format!(
        "{} nodes, {} quadratic tetrahedra, {} g; {} eigenvalue iterations",
        mesh.nodes.len(),
        mesh.tets.len(),
        super::result::short(found.total_mass * 1.0e6),
        found.iterations
    )];
    if let Some(bonds) = prepared.bonds() {
        notes.push(bonds);
    }
    if spec.supports.is_empty() {
        notes.push("no supports, so the six rigid-body modes at 0 Hz are left out".into());
    }
    let field = found.modes.first().map(|mode| {
        let displacement: Vec<[f64; 3]> =
            mode.shape.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
        let values: Vec<f64> = displacement
            .iter()
            .map(|d| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt())
            .collect();
        let top = values.iter().copied().fold(0.0, f64::max).max(1.0e-300);
        let displacement = displacement.iter().map(|d| d.map(|x| x / top)).collect();
        let values = values.iter().map(|v| v / top).collect();
        Field {
            title: format!("mode 1, {} Hz", super::result::short(mode.frequency)),
            unit: "relative".into(),
            values,
            displacement,
        }
    });
    Ok(Outcome {
        kind: "modal".into(),
        readings,
        notes,
        series: Vec::new(),
        checks: Vec::new(),
        aliases: Vec::new(),
        slices: Vec::new(),
        streams: Vec::new(),
        movie: None,
        field,
    })
}

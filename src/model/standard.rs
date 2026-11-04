use super::fastener::{Metric, named};
use anyhow::{Result, anyhow, bail};

pub const KINDS: &[(&str, &str)] = &[
    ("shcs", "ISO 4762"),
    ("fhcs", "ISO 10642"),
    ("hex", "ISO 4017"),
    ("nut", "ISO 4032"),
    ("washer", "ISO 7089"),
    ("insert", "heat-set insert"),
    ("standoff", "hex standoff"),
    ("bearing", "bearing"),
    ("dowel", "ISO 8734"),
    ("oring", "o-ring"),
];

pub const BEARINGS: &[(&str, f64, f64, f64)] = &[
    ("623", 3.0, 10.0, 4.0),
    ("624", 4.0, 13.0, 5.0),
    ("625", 5.0, 16.0, 5.0),
    ("626", 6.0, 19.0, 6.0),
    ("695", 5.0, 13.0, 4.0),
    ("688", 8.0, 16.0, 5.0),
    ("698", 8.0, 19.0, 6.0),
    ("608", 8.0, 22.0, 7.0),
    ("609", 9.0, 24.0, 7.0),
    ("6000", 10.0, 26.0, 8.0),
    ("6001", 12.0, 28.0, 8.0),
    ("6002", 15.0, 32.0, 9.0),
    ("6003", 17.0, 35.0, 10.0),
    ("6004", 20.0, 42.0, 12.0),
    ("6005", 25.0, 47.0, 12.0),
    ("6200", 10.0, 30.0, 9.0),
    ("6201", 12.0, 32.0, 10.0),
    ("6202", 15.0, 35.0, 11.0),
    ("6203", 17.0, 40.0, 12.0),
    ("6204", 20.0, 47.0, 14.0),
    ("6800", 10.0, 19.0, 5.0),
    ("6801", 12.0, 21.0, 5.0),
    ("6802", 15.0, 24.0, 5.0),
    ("6803", 17.0, 26.0, 5.0),
    ("6804", 20.0, 32.0, 7.0),
];

fn bearing(size: &str) -> Result<Standard> {
    let &(name, bore, outer, wide) = BEARINGS.iter().find(|b| b.0 == size).ok_or_else(|| {
        anyhow!(
            "unknown bearing `{size}`; known: {}",
            BEARINGS.iter().map(|b| b.0).collect::<Vec<_>>().join(", ")
        )
    })?;
    let wall = (outer - bore) / 2.0;
    let (seal_in, seal_out) = (bore + wall * 0.7, outer - wall * 0.7);
    let source = [
        format!("circle {outer}"),
        format!("ring: extrude {wide}"),
        "plane ring.end".into(),
        format!("circle {bore}"),
        "bore: cut thru".into(),
        format!("circle {seal_out}"),
        format!("circle {seal_in}"),
        format!("seal: cut {}", wide * 0.05),
        "plane ring.start".into(),
        format!("circle {seal_out}"),
        format!("circle {seal_in}"),
        format!("seal: cut {}", wide * 0.05),
        "material steel".into(),
    ]
    .join("\n");
    Ok(Standard {
        title: format!("{name} bearing {bore}x{outer}x{wide}"),
        source,
    })
}

fn oring(size: &str) -> Result<Standard> {
    let ring = crate::calc::tables::ring(size).ok_or_else(|| {
        anyhow!("no standard o-ring `{size}`; write an AS568 dash number like oring:214 or an ISO 3601 size like oring:40x2.65")
    })?;
    let source = [
        "plane XZ".to_string(),
        format!("circle {} at={},0", ring.cs, ring.id / 2.0 + ring.cs / 2.0),
        "ring: revolve 360 axis=y".into(),
        "material nitrile".into(),
    ]
    .join("\n");
    Ok(Standard {
        title: format!("{} {}x{}", ring.name(), ring.id, ring.cs),
        source,
    })
}

fn dowel(size: &str) -> Result<Standard> {
    let parts: Vec<f64> = size
        .split(['x', 'X'])
        .map(|p| p.parse::<f64>().ok().filter(|v| *v > 0.0))
        .collect::<Option<Vec<_>>>()
        .filter(|v| v.len() == 2)
        .ok_or_else(|| anyhow!("a dowel is written dowel:DxL, like dowel:4x16"))?;
    let (d, l) = (parts[0], parts[1]);
    if l <= d * 0.5 {
        bail!("a {d} dowel must be longer than {}", d * 0.5);
    }
    let source = [
        format!("circle {d}"),
        format!("pin: extrude {l}"),
        format!("chamfer {} pin.start&pin.side|pin.end&pin.side", d * 0.1),
        "material steel".into(),
    ]
    .join("\n");
    Ok(Standard {
        title: format!("ISO 8734 {d}x{l}"),
        source,
    })
}

pub struct Standard {
    pub title: String,
    pub source: String,
}

pub fn is_standard(spec: &str) -> bool {
    spec.split_once(':')
        .is_some_and(|(kind, _)| KINDS.iter().any(|(k, _)| *k == kind))
}

fn sized(size: &str, kind: &str) -> Result<(&'static Metric, Option<f64>)> {
    let (thread, length) = match size.split_once(['x', 'X']) {
        Some((thread, length)) => (
            thread,
            Some(
                length
                    .parse::<f64>()
                    .map_err(|_| anyhow!("`{length}` is not a length in `{kind}:{size}`"))?,
            ),
        ),
        None => (size, None),
    };
    Ok((named(thread)?, length))
}

fn hex_corners(across_flats: f64) -> f64 {
    across_flats / (std::f64::consts::PI / 6.0).cos()
}

pub fn standard(spec: &str) -> Result<Standard> {
    let (kind, size) = spec
        .split_once(':')
        .ok_or_else(|| anyhow!("a standard part is written kind:size, like shcs:M3x10"))?;
    let standard = KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, s)| *s)
        .ok_or_else(|| {
            anyhow!(
                "unknown standard part `{kind}`; known: {}",
                KINDS.iter().map(|k| k.0).collect::<Vec<_>>().join(", ")
            )
        })?;
    match kind {
        "bearing" => return bearing(size),
        "dowel" => return dowel(size),
        "oring" => return oring(size),
        _ => {}
    }
    let (m, length) = sized(size, kind)?;
    let screw = matches!(kind, "shcs" | "fhcs" | "hex" | "standoff");
    let length = match (screw, length) {
        (true, Some(l)) if l > 0.0 => Some(l),
        (true, _) => bail!("`{kind}` needs a length, like {kind}:{}x10", m.name),
        (false, Some(_)) => bail!("`{kind}` takes a size alone, like {kind}:{}", m.name),
        (false, None) => None,
    };
    let d = m.major;
    let lines: Vec<String> = match kind {
        "shcs" => {
            let (head, k) = m.socket_head;
            let (key, depth) = m.socket_key;
            let l = length.unwrap_or_default();
            vec![
                format!("circle {head}"),
                format!("head: extrude {k}"),
                "plane head.end".into(),
                format!("ngon {} 6", hex_corners(key)),
                format!("socket: cut {depth}"),
                "plane XY".into(),
                format!("circle {d}"),
                format!("shank: extrude -{l}"),
            ]
        }
        "fhcs" => {
            let h = m.flat_head_height();
            let l = length.unwrap_or_default();
            if l <= h {
                bail!(
                    "a {} flat head screw is longer than its {h:.2} head",
                    m.name
                );
            }
            let (key, depth) = m.flat_key;
            vec![
                format!("circle {}", m.flat_head),
                format!("head: extrude -{h} draft=45"),
                "plane XY".into(),
                format!("ngon {} 6", hex_corners(key)),
                format!("socket: cut {depth}"),
                format!("plane XY offset=-{h}"),
                format!("circle {d}"),
                format!("shank: extrude -{}", l - h),
            ]
        }
        "hex" => {
            let (across, k) = m.hex_head;
            let l = length.unwrap_or_default();
            vec![
                format!("ngon {} 6", hex_corners(across)),
                format!("head: extrude {k}"),
                format!("circle {d}"),
                format!("shank: extrude -{l}"),
            ]
        }
        "standoff" => {
            let (across, _) = m.nut;
            let l = length.unwrap_or_default();
            vec![
                format!("ngon {} 6", hex_corners(across)),
                format!("body: extrude {l}"),
                "plane body.end".into(),
                format!("bore: hole {} 0,0 fit=tap", m.name),
            ]
        }
        "nut" => {
            let (across, height) = m.nut;
            vec![
                format!("ngon {} 6", hex_corners(across)),
                format!("nut: extrude {height}"),
                "plane nut.end".into(),
                format!("circle {d}"),
                "bore: cut thru".into(),
            ]
        }
        "washer" => {
            let (outer, thick) = m.washer;
            vec![
                format!("circle {outer}"),
                format!("washer: extrude {thick}"),
                "plane washer.end".into(),
                format!("circle {}", m.clearance[0]),
                "bore: cut thru".into(),
            ]
        }
        _ => {
            let (_, long, outer) = m
                .insert
                .ok_or_else(|| anyhow!("there is no heat-set insert listed for {}", m.name))?;
            vec![
                format!("circle {outer}"),
                format!("insert: extrude -{long}"),
                "plane XY".into(),
                format!("circle {d}"),
                "bore: cut thru".into(),
            ]
        }
    };
    let material = if matches!(kind, "insert" | "standoff") {
        "brass"
    } else {
        "steel"
    };
    let title = match length {
        Some(l) => format!("{standard} {}x{l}", m.name),
        None => format!("{standard} {}", m.name),
    };
    let source = lines
        .into_iter()
        .chain([format!("material {material}")])
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Standard { title, source })
}

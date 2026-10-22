mod common;

use common::*;
use gcad::{export, render};

#[test]
fn step() {
    let model = build(&part("plate"));
    let path = scratch("plate.step");
    export::export(&model.solids(), &path).expect("exports");
    let text = std::fs::read_to_string(&path).expect("written");
    assert!(text.starts_with("ISO-10303-21;"));
    assert!(text.contains("MANIFOLD_SOLID_BREP") && text.contains("CLOSED_SHELL"));
}

#[test]
#[ignore = "needs OpenCascade: set GCAD_OCP_PYTHON to a python with OCP and run with --ignored"]
fn step_opens_in_opencascade() {
    let python = std::env::var("GCAD_OCP_PYTHON").expect("GCAD_OCP_PYTHON");
    let script = r#"
import sys
from OCP.Interface import Interface_Static
from OCP.STEPControl import STEPControl_Reader
from OCP.BRepCheck import BRepCheck_Analyzer
from OCP.BRepGProp import BRepGProp
from OCP.GProp import GProp_GProps
from OCP.TopExp import TopExp_Explorer
from OCP.TopAbs import TopAbs_FACE, TopAbs_EDGE
from OCP.TopoDS import TopoDS
from OCP.BRep import BRep_Tool
from OCP.BRepAdaptor import BRepAdaptor_Curve
from OCP.GeomAPI import GeomAPI_ProjectPointOnSurf
STEPControl_Reader()
Interface_Static.SetCVal_s("read.step.sequence", "")
r = STEPControl_Reader()
r.ReadFile(sys.argv[1])
r.TransferRoots()
shape = r.OneShape()
props = GProp_GProps()
BRepGProp.VolumeProperties_s(shape, props, 1e-7)
gap = 0.0
faces = TopExp_Explorer(shape, TopAbs_FACE)
while faces.More():
    face = TopoDS.Face_s(faces.Current())
    surface = BRep_Tool.Surface_s(face)
    edges = TopExp_Explorer(face, TopAbs_EDGE)
    while edges.More():
        curve = BRepAdaptor_Curve(TopoDS.Edge_s(edges.Current()))
        a, b = curve.FirstParameter(), curve.LastParameter()
        for i in range(23):
            projection = GeomAPI_ProjectPointOnSurf(curve.Value(a + (b - a) * i / 22), surface)
            if projection.NbPoints():
                gap = max(gap, projection.LowerDistance())
        edges.Next()
    faces.Next()
print(BRepCheck_Analyzer(shape).IsValid(), props.Mass(), gap)
"#;
    let inline = [
        ("rounded-box", "rect 30 20\nbase: extrude 10\nfillet 2 all"),
        (
            "rim-chain",
            "rect 40 30\nbase: extrude 3\nfillet 1 base.end&base.side",
        ),
        (
            "chamfered-box",
            "rect 30 20\nbase: extrude 10\nchamfer 2 all",
        ),
        (
            "revolve-cut",
            "rect 40 40\nbase: extrude 10\nplane XZ\nrect 4 4 at=10,10\nrevolve 360 axis=y mode=cut",
        ),
        ("rounded-poly", "poly 0,0 30,0 30,20 0,20 r=4\nextrude 5"),
        ("twisted", "rect 4 2\npath 0,0,0 0,0,20\nsweep twist=90"),
        ("helical-gear", "gear 16 2 helix=20\nextrude 8"),
        ("bevel-gear", "gear 16 2 cone=40\nextrude 5"),
        ("rack", "rack 6 2\nextrude 5"),
        (
            "weldment",
            "member pipe 33.7x2.6 0,0,0 300,0,0 300,0,200\nmember channel 50x25x3 0,-100,0 200,-100,0 300,-50,0 rotate=90",
        ),
        (
            "sheet-metal",
            "sheet 1 r=1\nrect 40 30\nbase: tab\nwalls: flange base.end&>X|base.end&>Y 10\nflange >X&walls.end 5 angle=60\nflange base.start&<X 4 inset=3\nflange base.end&<Y 6 angle=180",
        ),
        (
            "case-features",
            "rect 40 30\nbox: extrude 20\nshell 2 open=box.end\nlip >Z 3\nplane XY offset=2\nboss 7 12 10,5 hole=M3 fit=insert ribs=3\nplane >X\nvent 12 2 0,10 count=2 step=0,-4\nplane XY offset=2\nsnap 8 1.5 6 -10,0 dir=90",
        ),
        (
            "screw-holes",
            "rect 40 20\nb: extrude 8\nplane b.end\nhole M3 -10,0 cbore=M3\nhole M4 10,0 csink=M4\nhole M3 0,5 fit=insert",
        ),
        (
            "smooth-sweep",
            "circle 2\npath 0,0,0 10,0,10 20,0,0 smooth\nsweep",
        ),
        (
            "mixed-loft",
            "circle 20\nsection\nplane XY offset=10\nngon 20 6\nsection\nloft",
        ),
        (
            "partly-rounded",
            "rect 20 20\nbase: extrude 20\nfillet 2 base.end&base.side|>X&>Y",
        ),
        (
            "counterbored",
            "rect 40 30\nbase: extrude 10\nplane base.end\nhole 3.2 10,0 cbore=6,3\nhole 3.2 -10,0 csink=6.4,90",
        ),
        ("mirrored", "rect 20 10 at=10,0\nextrude 5\nmirror YZ"),
        (
            "lofted",
            "rect 20 20\nsection\nplane XY offset=10\ncircle 10\nsection\nloft",
        ),
        ("spring", "circle 2\nhelix r=10 pitch=5 turns=2\nsweep"),
        (
            "drafted",
            "rect 20 20\nbase: extrude 10\ndraft 5 base.side neutral=base.start",
        ),
        (
            "shelled",
            "rect 40 30 r=4\nbase: extrude 20\nshell 2 open=base.end",
        ),
        (
            "lettered",
            "rect 60 15\nbase: extrude 2\nplane base.end\ntext CAD size=8 at=-10,-4\nextrude 1",
        ),
        (
            "wrapped",
            "circle 40\nbase: extrude 20\nplane XZ\ntext HELLO size=8 at=-14,6\nwrap base.side depth=0.6",
        ),
        (
            "threaded",
            "circle 6\nrod: extrude 3\nthread M6 on=rod.side",
        ),
        (
            "thread-tip",
            "circle 6\nrod: extrude 6\nchamfer 1 rod.end&rod.side|rod.start&rod.side\nthread M6 on=rod.side",
        ),
        (
            "countersunk-tapped",
            "rect 20 20\nbase: extrude 4\nplane base.end\nh: hole 5 0,0 csink=7,90 thread=M6\nthread M6 on=h.side",
        ),
        (
            "tapped",
            "rect 20 20\nbase: extrude 4\nplane base.end\nh: hole 5 0,0 thread=M6\nthread M6 on=h.side",
        ),
        (
            "l-rounded",
            "poly 0,0 20,0 20,10 10,10 10,20 0,20\nextrude 10\nfillet 1 all",
        ),
        (
            "pocket-rounded",
            "rect 40 40\nbase: extrude 10\nplane base.end\nrect 20 20\ncut 5\nfillet 1 all",
        ),
        (
            "sloted",
            "slot 30 10\nextrude 3\nplane XY offset=3\nellipse 10 6\nextrude 2",
        ),
    ];
    let named = ["plate", "bracket", "pipe", "ring", "enclosure"].map(|name| (name, part(name)));
    for (name, source) in named.iter().map(|(n, s)| (*n, s.as_str())).chain(inline) {
        let model = build(source);
        let path = scratch(&format!("{name}.step"));
        export::export(&model.solids(), &path).expect("exports");
        let output = std::process::Command::new(&python)
            .args(["-c", script, &path])
            .output()
            .expect("python runs");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut words = stdout.split_whitespace();
        assert_eq!(
            words.next(),
            Some("True"),
            "{name}: OpenCascade rejects the shape: {stdout} {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let occt: f64 = words.next().and_then(|w| w.parse().ok()).expect("a volume");
        let gap: f64 = words.next().and_then(|w| w.parse().ok()).expect("a gap");
        let ours = volume(&model);
        assert!(
            (occt - ours).abs() < ours * 0.001,
            "{name}: OpenCascade volume {occt}, ours {ours}"
        );
        let size = model
            .solids()
            .iter()
            .map(|solid| gcad::geometry::bounds(solid).diameter())
            .fold(0.0, f64::max);
        assert!(
            gap < size * 1.0e-6,
            "{name}: an edge strays {gap} from its face, over a millionth of the part's {size}"
        );
    }
}

#[test]
fn stl() {
    let model = build("rect 10 10\nextrude 5");
    let path = scratch("box.stl");
    export::export(&model.solids(), &path).expect("exports");
    let bytes = std::fs::read(&path).expect("written");
    let triangles = u32::from_le_bytes(bytes[80..84].try_into().expect("header")) as usize;
    assert_eq!(bytes.len(), 84 + triangles * 50);
    let vertex = |t: usize, k: usize| -> [f64; 3] {
        let at = 84 + t * 50 + 12 + k * 12;
        [0, 4, 8].map(|o| {
            f32::from_le_bytes(bytes[at + o..at + o + 4].try_into().expect("float")) as f64
        })
    };
    let signed: f64 = (0..triangles)
        .map(|t| {
            let (a, b, c) = (vertex(t, 0), vertex(t, 1), vertex(t, 2));
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum();
    assert!((signed - 500.0).abs() < 1.0e-3, "STL encloses {signed}");
}

#[test]
fn simulation_parts_export() {
    let dir = std::path::PathBuf::from(scratch("simulation_parts_export"));
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("block.gcad"), "rect 10 10\nextrude 10\n").expect("part");
    std::fs::write(
        dir.join("pair.gasm"),
        "part a block.gcad\npart b_screw block.gcad\nmove b_screw 20,0,0\n",
    )
    .expect("assembly");
    let spec = dir.join("wind.gsim");
    std::fs::write(&spec, "sim flow\npart pair.gasm\nleave *screw\nwind 10\n").expect("spec");
    let run = gcad::model::run_path(&spec, &[], None).expect("runs");
    let out = dir.join("parts.stl");
    export::export_model(&run.model, &spec, out.to_str().expect("utf-8")).expect("exports");
    let bytes = std::fs::read(&out).expect("written");
    let triangles = u32::from_le_bytes(bytes[80..84].try_into().expect("header")) as usize;
    let xs: Vec<f32> = (0..triangles)
        .flat_map(|t| (0..3).map(move |k| 84 + t * 50 + 12 + k * 12))
        .map(|at| f32::from_le_bytes(bytes[at..at + 4].try_into().expect("float")))
        .collect();
    assert!(triangles > 0);
    assert!(
        xs.iter().all(|&x| x < 10.5),
        "the left-out part was exported"
    );
}

#[test]
fn png() {
    let model = build(&part("plate"));
    let path = scratch("plate.png");
    render::render(&model.solids(), &path).expect("renders");
    let image = image::open(&path).expect("a png").to_rgb8();
    assert_eq!(image.dimensions(), (1200, 1200));
    let background = *image.get_pixel(5, 5);
    let drawn = image.pixels().filter(|p| **p != background).count();
    assert!(drawn > 50_000, "only {drawn} pixels drawn");
}

#[test]
fn step_import() {
    let model = build(&part("plate"));
    let path = scratch("import.step");
    export::export(&model.solids(), &path).expect("exports");
    assert_volume(&build(&format!("import {path}")), volume(&model), 0.0005);
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn step_import_from_opencascade() {
    let file = fixture("occt-two-solids.step");
    assert_volume(
        &build(&format!("import {file} solid=1")),
        4000.0 - std::f64::consts::PI * 16.0 * 10.0,
        5.0e-4,
    );
    assert_volume(
        &build(&format!("import {file} solid=2")),
        std::f64::consts::PI * 25.0 * 10.0,
        5.0e-4,
    );
}

#[test]
fn step_import_boxes_what_it_cannot_read() {
    let file = fixture("open-surface.step");
    let source = format!("import {file}");
    assert_volume(&build(&source), 6.0, 1.0e-9);
    let text = summary(&source);
    assert!(
        text.contains("a box stands in") && text.contains("watertight"),
        "{text}"
    );
}

#[test]
fn step_import_reads_a_revolved_circle() {
    let source = format!("import {}", fixture("revolved-groove.step"));
    let groove = 5.0 * std::f64::consts::PI.powi(2) - 4.0 * std::f64::consts::PI / 3.0;
    assert_volume(
        &build(&source),
        250.0 * std::f64::consts::PI - groove,
        1.0e-3,
    );
    let text = summary(&source);
    assert!(!text.contains("stands in"), "{text}");
}

#[test]
fn step_import_leaves_unreadable_cavities_solid() {
    let source = format!("import {}", fixture("void-open.step"));
    assert_volume(&build(&source), 992.0, 1.0e-9);
    let text = summary(&source);
    assert!(
        text.contains("1 of its cavities gcad cannot read is left solid"),
        "{text}"
    );
}

#[test]
fn step_import_asks_which_solid() {
    let error = failure(&format!("import {}", fixture("occt-two-solids.step")));
    assert!(
        error.contains("holds 2 solids, pick one with `solid=`"),
        "{error}"
    );
}

#[test]
fn obj() {
    let path = scratch("box.obj");
    export::export(&build("rect 10 10\nextrude 5").solids(), &path).expect("exports");
    assert!(
        std::fs::read_to_string(&path)
            .expect("written")
            .lines()
            .any(|l| l.starts_with("f "))
    );
}

#[test]
fn three_mf() {
    let path = scratch("box.3mf");
    export::export(&build("rect 10 10\nextrude 5").solids(), &path).expect("exports");
    assert_eq!(&std::fs::read(&path).expect("written")[..2], b"PK");
}

#[test]
fn three_mf_names_the_bodies() {
    let model = build("body one\nrect 10 10\nextrude 5\nbody two\nrect 6 6\nextrude 3\n");
    let path = scratch("named.3mf");
    export::export_model(&model, std::path::Path::new("multi.gcad"), &path).expect("exports");
    let bytes = std::fs::read(&path).expect("written");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("name=\"one\""), "{text}");
    assert!(text.contains("name=\"two\""), "{text}");
    assert!(!text.contains("body0"), "objects still named by index");
}

#[test]
fn assembly_step_keeps_multi_shell_parts() {
    let dir = std::env::temp_dir()
        .join("gcad-tests")
        .join("asm_multi_shell");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(
        dir.join("twoshell.gcad"),
        "body left\nrect 10 10\nextrude 5\nbody right\nplane XY offset=30\nrect 10 10\nextrude 5\ncombine into=left from=right\n",
    )
    .expect("write");
    std::fs::write(dir.join("plain.gcad"), "rect 10 10\nextrude 5\n").expect("write");
    std::fs::write(
        dir.join("asm.gasm"),
        "part a twoshell.gcad\npart b plain.gcad\n",
    )
    .expect("write");
    let run = gcad::model::run_path(&dir.join("asm.gasm"), &[], None).expect("runs");
    let out = dir.join("out.step");
    export::export_model(
        &run.model,
        &dir.join("asm.gasm"),
        out.to_str().expect("utf8"),
    )
    .expect("exports");
    let text = std::fs::read_to_string(&out).expect("written");
    let nauo = text.matches("NEXT_ASSEMBLY_USAGE_OCCURRENCE").count();
    let manifold = text.matches("MANIFOLD_SOLID_BREP").count();
    let voids = text.matches("BREP_WITH_VOIDS").count();
    assert_eq!(nauo, 2, "assembly tree lost: {text}");
    assert_eq!(voids, 1, "multi-shell part not a BREP_WITH_VOIDS: {text}");
    assert_eq!(
        manifold, 1,
        "single-shell part not a MANIFOLD_SOLID_BREP: {text}"
    );
}

#[test]
fn drawing() {
    let path = scratch("plate.svg");
    export::export(&build(&part("plate")).solids(), &path).expect("exports");
    assert!(
        std::fs::read_to_string(&path)
            .expect("written")
            .contains("<path")
    );
}

#[test]
fn stl_import() {
    let path = scratch("import.stl");
    export::export(&build("rect 10 10\nextrude 5").solids(), &path).expect("exports");
    assert_volume(&build(&format!("import {path}")), 500.0, 1.0e-6);
}

#[test]
fn step_colours() {
    let path = scratch("red.step");
    export::export_coloured(&build("rect 10 10\nextrude 5\ncolor red").parts(), &path)
        .expect("exports");
    assert!(
        std::fs::read_to_string(&path)
            .expect("written")
            .contains("COLOUR_RGB")
    );
}

fn screwed_plate(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(scratch(name));
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(
        dir.join("plate.gcad"),
        "rect 40 20\nbase: extrude 5\nplane base.end\nholes: hole 3 -10,0 10,0\ncolor #3a6ea5\n",
    )
    .expect("plate");
    std::fs::write(
        dir.join("pin.gcad"),
        "circle 5\nhead: extrude 1\ncircle 3\npin: extrude -5\ncolor red\n",
    )
    .expect("pin");
    let path = dir.join("kit.gasm");
    std::fs::write(
        &path,
        "part plate plate.gcad\npart a pin.gcad\npart b pin.gcad\nconcentric a:pin.side plate:holes.side near=-10,0,5\nflush a:head.start plate:base.end\nconcentric b:pin.side plate:holes.side near=10,0,5\nflush b:head.start plate:base.end\n",
    )
    .expect("assembly");
    path
}

#[test]
fn step_assembly() {
    let path = screwed_plate("step_assembly");
    let run = gcad::model::run_path(&path, &[], None).expect("runs");
    let out = scratch("kit.step");
    export::export_model(&run.model, &path, &out).expect("exports");
    let text = std::fs::read_to_string(&out).expect("written");
    assert_eq!(text.matches("NEXT_ASSEMBLY_USAGE_OCCURRENCE(").count(), 3);
    assert_eq!(text.matches("= MANIFOLD_SOLID_BREP(").count(), 2);
    assert!(text.contains("PRODUCT('kit'") && text.contains("PRODUCT('pin'"));
}

#[test]
#[ignore = "needs OpenCascade: set GCAD_OCP_PYTHON to a python with OCP and run with --ignored"]
fn step_assembly_opens_in_opencascade() {
    let python = std::env::var("GCAD_OCP_PYTHON").expect("GCAD_OCP_PYTHON");
    let path = screwed_plate("step_assembly_occt");
    let run = gcad::model::run_path(&path, &[], None).expect("runs");
    let out = scratch("kit_occt.step");
    export::export_model(&run.model, &path, &out).expect("exports");
    let script = r#"
import sys
from OCP.STEPCAFControl import STEPCAFControl_Reader
from OCP.TDocStd import TDocStd_Document
from OCP.TCollection import TCollection_ExtendedString
from OCP.XCAFDoc import XCAFDoc_DocumentTool
from OCP.TDF import TDF_LabelSequence, TDF_Label
from OCP.TDataStd import TDataStd_Name
from OCP.BRepCheck import BRepCheck_Analyzer
from OCP.BRepGProp import BRepGProp
from OCP.GProp import GProp_GProps
doc = TDocStd_Document(TCollection_ExtendedString("doc"))
r = STEPCAFControl_Reader()
r.SetNameMode(True)
r.ReadFile(sys.argv[1])
r.Transfer(doc)
tool = XCAFDoc_DocumentTool.ShapeTool_s(doc.Main())
def name(label):
    n = TDataStd_Name()
    return n.Get().ToExtString() if label.FindAttribute(TDataStd_Name.GetID_s(), n) else "?"
roots = TDF_LabelSequence()
tool.GetFreeShapes(roots)
top = roots.Value(1)
parts = TDF_LabelSequence()
tool.GetComponents_s(top, parts)
print(roots.Length(), name(top), parts.Length())
for i in range(1, parts.Length() + 1):
    part = parts.Value(i)
    used = TDF_Label()
    tool.GetReferredShape_s(part, used)
    shape = tool.GetShape_s(part)
    props = GProp_GProps()
    BRepGProp.VolumeProperties_s(shape, props, 1e-7)
    c = props.CentreOfMass()
    print(name(part), name(used), BRepCheck_Analyzer(shape).IsValid(), *(f"{v:.2f}".replace("-0.00", "0.00") for v in (c.X(), c.Y(), c.Z())))
"#;
    let output = std::process::Command::new(&python)
        .args(["-c", script, &out])
        .output()
        .expect("python runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        [
            "1 kit 3",
            "plate plate True 0.00 0.00 2.50",
            "a pin True -10.00 0.00 3.57",
            "b pin True 10.00 0.00 3.57",
        ],
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn drawing_dimensions_and_section() {
    let model =
        build("rect 20 10\nbase: extrude 10\nplane base.end\nholes: hole 4 0,0 6,0\nhole 2 -6,0");
    let svg = gcad::drawing::drawing_with(
        &model.parts(),
        Some(gcad::drawing::Section { axis: 1, at: 0.0 }),
    );
    assert!(svg.contains(">20</text>") && svg.contains(">10</text>"));
    assert!(svg.contains(">2× ⌀4</text>") && svg.contains(">⌀2</text>"));
    assert!(svg.contains("section y=0") && svg.contains("class=\"cut\""));
    let solid = model.solid.as_ref().expect("solid");
    let outlines =
        gcad::drawing::section_outlines(solid, gcad::drawing::Section { axis: 1, at: 0.0 });
    let area: f64 = outlines
        .iter()
        .map(|outline| {
            outline
                .windows(2)
                .map(|w| w[0].x * w[1].z - w[1].x * w[0].z)
                .sum::<f64>()
                .abs()
                / 2.0
        })
        .sum();
    assert_eq!(outlines.len(), 4);
    assert!(
        (area - (200.0 - 10.0 * (4.0 + 4.0 + 2.0))).abs() < 0.01,
        "{area}"
    );
    assert_eq!(
        "y=0".parse(),
        Ok(gcad::drawing::Section { axis: 1, at: 0.0 })
    );
    assert!("w=1".parse::<gcad::drawing::Section>().is_err());
}

#[test]
fn assembly_drawing_lists_parts() {
    let path = screwed_plate("assembly_drawing");
    let run = gcad::model::run_path(&path, &[], None).expect("runs");
    let out = scratch("kit.svg");
    export::export_model(&run.model, &path, &out).expect("exports");
    let svg = std::fs::read_to_string(&out).expect("written");
    for words in [">item<", ">plate<", ">pin<", ">2<"] {
        assert!(svg.contains(words), "{words}");
    }
    assert_eq!(svg.matches("class=\"balloon\"").count(), 2);
    assert!(svg.contains("class=\"parts-list\""));
    assert!(svg.contains(">2× ⌀3<"));
}

#[test]
fn step_import_places_instances() {
    let file = fixture("occt-instances.step");
    let error = failure(&format!("import {file}"));
    assert!(error.contains("4: `pin:3`"), "{error}");
    let third = build(&format!("import {file} solid=4"));
    assert_bounds(&third, [28.0, 8.0, 5.0], [32.0, 12.0, 15.0]);
}

#[test]
fn drawing_notes_fits_and_taps() {
    let model = build(
        "rect 40 20\nb: extrude 8\nplane b.end\nhole M3 -10,0 cbore=M3\nhole 6 10,0 fit=H7\nhole M4 0,5 fit=tap",
    );
    let svg = gcad::drawing::drawing_noted(&model.parts(), None, &model.hole_notes);
    for callout in ["⌀6 H7", "⌀3.3 M4 tapped", "⌀3.4<", "⌀6<"] {
        assert!(svg.contains(callout), "missing {callout}");
    }
}

#[test]
fn flat_pattern_dxf() {
    let path = scratch("tray.dxf");
    let model = build(
        "sheet 1 r=1\nrect 40 30\nbase: tab\nplane base.end\nhole 3 0,0\nflange base.end&base.side 10",
    );
    gcad::export::export_model(&model, std::path::Path::new("tray.gcad"), &path).expect("exports");
    let dxf = std::fs::read_to_string(&path).expect("written");
    let entities: Vec<&str> = dxf.lines().collect();
    let layers: Vec<&str> = entities
        .windows(4)
        .filter(|w| w[0] == "LINE" && w[1] == "8")
        .map(|w| w[2])
        .collect();
    assert_eq!(layers.iter().filter(|l| **l == "BEND").count(), 4);
    assert!(layers.iter().filter(|l| **l == "OUTLINE").count() >= 12 + 8);
    assert!(dxf.ends_with("EOF\n"));
    let plain = build("rect 10 10\nextrude 2");
    assert!(
        gcad::export::export_model(&plain, std::path::Path::new("p.gcad"), &path)
            .expect_err("not sheet metal")
            .to_string()
            .contains("flat pattern")
    );
}

fn drawing_files(name: &str, sheet: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(scratch(name));
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(
        dir.join("wall.gcad"),
        "member bar 44x150 0,0,0 2400,0,0\nmember bar 44x150 22,0,22 22,0,1200 up=0,1,0\nmember bar 44x150 622,0,22 622,0,1200 up=0,1,0\nmember bar 44x150 1222,0,22 1222,0,1200 up=0,1,0\nbody slab\ncolor #a0a0a0\nplane XY offset=-150\nrect 2400 600 at=1200,0\nextrude 150\n",
    )
    .expect("part");
    let path = dir.join("wall.gdwg");
    std::fs::write(&path, sheet).expect("drawing");
    path
}

#[test]
fn drawing_sheets() {
    let path = drawing_files(
        "drawing_sheets",
        "part wall.gcad\ntitle \"Test wall\"\nsheet \"Frame\" size=A3 scale=1:20\nview profile=\"bar 44x150\" lengths=1 from=-y label=\"frame\"\ndims profile=\"bar 44x150\" along=h across=1\nlevel 0 \"slab\"\ncallout 622,0,600 \"stud\" to=20,10\nsection slab from=+x at=1200 scale=1:10\nnote \"one|two\"\ncutlist summary=1\ncutlist\n",
    );
    let drawing = gcad::drafting::run(&path, &[]).expect("runs");
    assert!(
        drawing.failed().is_none(),
        "{:?}",
        drawing.failed().map(|(l, e)| format!("{}: {e:#}", l.text))
    );
    assert_eq!(drawing.sheets.len(), 1);
    let svg = gcad::drafting::svg(&drawing.sheets[0]);
    for words in [
        ">600<",
        ">+0 slab<",
        ">stud<",
        ">1178<",
        ">Test wall<",
        ">1 of 1<",
        ">44x150<",
    ] {
        assert!(svg.contains(words), "the sheet lacks {words}");
    }
    let pdf = gcad::drafting::pdf(&drawing.sheets);
    assert!(pdf.starts_with(b"%PDF-1.4"));
    assert!(String::from_utf8_lossy(&pdf).contains("/Count 1"));
}

#[test]
fn drawing_nests_panels_on_stock() {
    let dir = std::path::PathBuf::from(scratch("drawing_nest"));
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(
        dir.join("panels.gcad"),
        "rect 18 380 at=9,190\nextrude 1900\nbody b\nrect 744 380 at=500,190\nextrude 18\nbody c\nrect 744 380 at=1500,190\nextrude 18\nbody d\nrect 744 100 at=2500,50\nextrude 18\nbody thick\nrect 300 300 at=4000,0\nextrude 40\n",
    )
    .expect("part");
    let path = dir.join("panels.gdwg");
    std::fs::write(
        &path,
        "part panels.gcad\nsheet \"Stock\" size=A3\nnest !thick stock=2400x600 gap=4\n",
    )
    .expect("drawing");
    let drawing = gcad::drafting::run(&path, &[]).expect("runs");
    assert!(
        drawing.failed().is_none(),
        "{:?}",
        drawing.failed().map(|(l, e)| format!("{}: {e:#}", l.text))
    );
    assert_eq!(
        drawing.sheets.len(),
        2,
        "1900x380 + 744x100 on one, two 744x380 on another"
    );
    let svg = gcad::drafting::svg(&drawing.sheets[0]);
    for words in [">1900 x 380<", "rip at y 380", "stock 1 of 2"] {
        assert!(svg.contains(words), "the sheet lacks {words}");
    }
    std::fs::write(
        &path,
        "part panels.gcad\nsheet \"Stock\" size=A3\nnest stock=2400x600\n",
    )
    .expect("drawing");
    let drawing = gcad::drafting::run(&path, &[]).expect("runs");
    let (_, error) = drawing.failed().expect("mixed thicknesses");
    assert!(format!("{error:#}").contains("thickness="), "{error:#}");
}

#[test]
fn drawing_view_that_does_not_fit() {
    let path = drawing_files(
        "drawing_too_big",
        "part wall.gcad\nsheet \"Frame\" size=A4 scale=1:1\nview from=-y\n",
    );
    let drawing = gcad::drafting::run(&path, &[]).expect("runs");
    let (_, error) = drawing.failed().expect("the view is too big");
    assert!(format!("{error:#}").contains("does not fit"), "{error:#}");
}

#[test]
fn drawing_labels_sit_clear_of_the_view() {
    let dir = std::path::PathBuf::from(scratch("drawing_labels"));
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(
        dir.join("plate.gcad"),
        "rect 40 30\nbase: extrude 6\nplane base.end\nbig: hole 12 -10,0\nsmall: hole 4 12,8 12,-8\n",
    )
    .expect("part");
    let path = dir.join("plate.gdwg");
    std::fs::write(
        &path,
        "part plate.gcad\nsheet \"Plate\" size=A4 scale=2:1\nview from=+z place=100,100\nlabel main:big.side\nlabel main:small.side\nlabel \"edge\" main:base.end&base.side\n",
    )
    .expect("drawing");
    let drawing = gcad::drafting::run(&path, &[]).expect("runs");
    assert!(
        drawing.failed().is_none(),
        "{:?}",
        drawing.failed().map(|(l, e)| format!("{}: {e:#}", l.text))
    );
    let svg = gcad::drafting::svg(&drawing.sheets[0]);
    for words in [">⌀12<", ">2× ⌀4<", ">edge<"] {
        assert!(svg.contains(words), "the sheet lacks {words}");
    }
}

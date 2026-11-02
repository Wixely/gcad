use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};
use gcad::model::{Run, run_path};
use gcad::{export, render, select};

#[derive(Parser)]
#[command(
    about = "Parametric CAD written like G-code: one operation per line",
    version = env!("GCAD_BUILD_VERSION"),
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Open this file in the viewer (the default when no command is given)
    file: Option<String>,
    /// Set a variable, overriding the file's `let`, as name=value (repeatable)
    #[arg(long = "set", global = true, value_parser = parse_set)]
    set: Vec<(String, f64)>,
}

fn parse_price(text: &str) -> std::result::Result<u64, String> {
    let euro: f64 = text
        .trim_start_matches('€')
        .parse()
        .map_err(|_| format!("`{text}` is not a price in euro an hour, like 2.50"))?;
    if !(0.0..=1000.0).contains(&euro) {
        return Err("the price should be from 0 (free runners only) to 1000 euro an hour".into());
    }
    Ok((euro * 100.0).round() as u64)
}

fn parse_set(text: &str) -> std::result::Result<(String, f64), String> {
    let (name, value) = text.split_once('=').ok_or("write it as name=value")?;
    let value = value
        .parse::<f64>()
        .map_err(|_| format!("`{value}` is not a number"))?;
    Ok((name.to_string(), value))
}

#[derive(Subcommand)]
enum Command {
    #[cfg(feature = "game-assets")]
    /// Clean abandoned, marked FBX stages; skip stages held by gcad or a helper
    AssetFbxRecover { directory: std::path::PathBuf },
    #[cfg(feature = "game-assets")]
    /// Experimental GLB-to-FBX/PNG package conversion using the reviewed native helper
    AssetFbxExperimental {
        /// Canonical gcad-exported GLB with provenance
        input: std::path::PathBuf,
        /// New package directory (must not exist)
        output: std::path::PathBuf,
        #[arg(long)]
        helper: std::path::PathBuf,
        /// Deadline in seconds for each helper process
        #[arg(long, default_value_t = 120)]
        timeout_seconds: u64,
    },
    #[cfg(feature = "game-assets")]
    /// Inspect an explicitly configured native FBX helper (does not export)
    AssetFbxHelper {
        #[arg(long)]
        helper: std::path::PathBuf,
    },
    #[cfg(feature = "game-assets")]
    /// Replace saved faces' skin weights in a new derived GLB
    AssetFaceWeights {
        file: String,
        node: String,
        selection: String,
        output: String,
        /// Existing skin joint and positive weight, JOINT=VALUE (repeat up to four)
        #[arg(long, required = true, value_parser = parse_set)]
        influence: Vec<(String, f64)>,
        /// Split selected boundary tuples, or reject shared selected/unselected vertices
        #[arg(long, value_parser = ["reject-shared", "split"])]
        boundary: String,
    },
    #[cfg(feature = "game-assets")]
    /// Transform saved faces' UVs and preserve skin weights in a new derived GLB
    AssetFaceUv {
        file: String,
        node: String,
        selection: String,
        output: String,
        #[arg(long, default_value = "1,1", allow_hyphen_values = true)]
        scale: String,
        #[arg(long, default_value_t = 0., allow_hyphen_values = true)]
        rotation: f64,
        #[arg(long, default_value = "0,0", allow_hyphen_values = true)]
        offset: String,
    },
    #[cfg(feature = "game-assets")]
    /// Assign surface settings to saved source faces and write a new derived GLB
    AssetFaceMaterial {
        file: String,
        node: String,
        selection: String,
        output: String,
        /// Surface KEY=VALUE override; repeat for additional settings
        #[arg(long, required = true)]
        surface: Vec<String>,
    },
    #[cfg(feature = "game-assets")]
    /// Validate a saved face selector and return prepared node triangle indices
    AssetFaceResolve {
        file: String,
        node: String,
        selection: String,
    },
    #[cfg(feature = "game-assets")]
    /// Capture a source-bound face selector into a new JSON record
    AssetFaceBind {
        file: String,
        source: String,
        selector: String,
        output: String,
        /// Select one body from a wildcard assembly source
        #[arg(long)]
        body: Option<String>,
    },
    #[cfg(feature = "game-assets")]
    /// Check a saved face selection against current CAD inputs without updating it
    AssetFaceCheck {
        file: String,
        source: String,
        selection: String,
        #[arg(long)]
        body: Option<String>,
    },
    #[cfg(feature = "game-assets")]
    /// Quantise skin weights to UNORM16 in a new GLB with explicit sampled limits
    AssetQuantizeWeights {
        file: String,
        output: String,
        /// Maximum sampled position error in millimetres
        #[arg(long)]
        max_error: f64,
        /// Maximum sampled normal/tangent angle error in degrees
        #[arg(long)]
        max_angle: f64,
        #[arg(long, default_value_t = 17)]
        samples: usize,
    },
    #[cfg(feature = "game-assets")]
    /// Remove unused skin palette entries into a new GLB, preserving every scene node
    AssetPruneJoints {
        file: String,
        output: String,
        /// Additional scene nodes whose skin-palette entries must be retained
        #[arg(long, value_delimiter = ',')]
        keep: Vec<String>,
    },
    #[cfg(feature = "game-assets")]
    /// Dry-run GLB encoding and report export capabilities as JSON
    AssetCapabilities { file: String },
    #[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
    /// Export a derived GLB and density report, preserving authored texture files
    AssetNormalizeTextures {
        file: String,
        output: String,
        #[arg(long, default_value_t = 512.0)]
        pixels_per_metre: f64,
        #[arg(long, default_value_t = 2048)]
        max_dimension: u32,
    },
    #[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
    /// Export a GPU-rendered transparent sprite sheet and frame metadata from a JSON recipe
    AssetSprites {
        file: String,
        recipe: String,
        output: String,
    },
    #[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
    /// Preview an authored asset, or render a software diagnostic PNG
    AssetPreview {
        file: String,
        #[arg(long)]
        clip: Option<String>,
        #[arg(long, default_value_t = 0.0)]
        time: f64,
        #[arg(long)]
        snapshot: Option<String>,
    },
    #[cfg(feature = "game-assets")]
    /// Export labelled UV islands and matching model views for painting
    #[cfg(not(target_arch = "wasm32"))]
    AssetPaintGuide {
        file: String,
        node: String,
        output: String,
        #[arg(long, default_value_t = 0)]
        primitive: usize,
        #[arg(long, default_value_t = 1024)]
        resolution: u32,
        #[arg(long, default_value_t = 8)]
        padding: u32,
    },
    #[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
    /// Validate a painted image against its UV guide and extend island-edge padding
    AssetPaintApply { file: String, guide: String, painted: String, output: String },
    #[cfg(feature = "game-assets")]
    /// Emit JSON geometry counts and sampled skin-deformation diagnostics
    AssetReport {
        file: String,
        /// Also validate CPU shading directions; singular blended transforms fail
        #[arg(long)]
        shading: bool,
        /// Regular samples per clip, supplemented by keys and STEP left limits
        #[arg(long, default_value_t = 17)]
        samples: usize,
    },
    #[cfg(feature = "game-assets")]
    /// Reduce a selected clip against sampled world-space error limits; create a new GLB
    AssetReduce {
        file: String,
        output: String,
        #[arg(long)]
        clip: String,
        #[arg(long)]
        max_error: f64,
        #[arg(long)]
        max_angle: f64,
        #[arg(long, default_value_t = 4)]
        subdivisions: usize,
        #[arg(long, default_value_t = 1000)]
        max_trials: usize,
    },
    #[cfg(feature = "game-assets")]
    /// Export an in-place clip and explicit root-motion metadata in a new GLB
    AssetRootMotion {
        file: String,
        node: String,
        output: String,
        #[arg(long)]
        clip: String,
        #[arg(long)]
        in_place: String,
        /// Extract explicit CAD axes, e.g. xy or xyz (Z is vertical)
        #[arg(long, required_unless_present = "rigid", conflicts_with = "rigid")]
        axes: Option<String>,
        /// Extract all translation and quaternion rotation with turning loop composition
        #[arg(long)]
        rigid: bool,
    },
    #[cfg(feature = "game-assets")]
    /// Derive a sampled, node-specific LOD into a new GLB
    AssetLod {
        file: String,
        node: String,
        output: String,
        #[arg(long, default_value_t = 0)]
        primitive: usize,
        #[arg(long)]
        triangles: usize,
        #[arg(long)]
        max_error: f64,
        #[arg(long)]
        max_uv_error: f64,
        #[arg(long)]
        max_surface_error: Option<f64>,
        #[arg(long)]
        max_surface_p95: Option<f64>,
        #[arg(long)]
        max_silhouette_difference: Option<f64>,
        #[arg(long)]
        max_angle: f64,
        #[arg(long, default_value_t = 17)]
        samples: usize,
        #[arg(long, default_value_t = 1000)]
        max_trials: usize,
        #[arg(long, value_delimiter = ',')]
        lock: Vec<usize>,
        /// Bake original rest normals onto this LOD at SIZE x SIZE resolution
        #[arg(long, requires = "bake_distance")]
        bake_normals: Option<u32>,
        /// Maximum projection distance in target mesh-local millimetres
        #[arg(long, requires = "bake_normals")]
        bake_distance: Option<f64>,
        #[arg(long, default_value_t = 2)]
        bake_padding: u8,
    },
    #[cfg(feature = "game-assets")]
    /// Bake source rest-pose normals into a target primitive and create a new GLB
    AssetBakeNormals {
        file: String,
        node: String,
        source: String,
        source_node: String,
        output: String,
        #[arg(long, default_value_t = 0)]
        primitive: usize,
        #[arg(long, default_value_t = 0)]
        source_primitive: usize,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        /// Maximum projection distance in target mesh-local millimetres
        #[arg(long)]
        max_distance: f64,
        #[arg(long, default_value_t = 0.0)]
        min_normal_dot: f64,
        #[arg(long, default_value_t = 0.0)]
        max_missed_fraction: f64,
        #[arg(long, default_value_t = 2)]
        padding: u8,
    },
    #[cfg(feature = "game-assets")]
    /// Explicitly create a new skin weight file; never overwrites an existing file
    AssetBind {
        file: String,
        node: String,
        output: String,
        /// One rigid joint, two with --blend, or ordered joints with --chain
        #[arg(long, value_delimiter = ',', required = true)]
        joints: Vec<String>,
        /// Mesh-local linear blend interval in mm: x,START,END (or y/z)
        #[arg(long, conflicts_with = "chain")]
        blend: Option<String>,
        /// Mesh-local piecewise blend knots in mm: AXIS,KNOT0,KNOT1,...
        #[arg(long)]
        chain: Option<String>,
        /// Explicit peer skin nodes to repair later; normal check/export remain strict
        #[arg(long = "defer-skin", value_delimiter = ',')]
        deferred_skins: Vec<String>,
    },
    /// Run every line and report what each one did
    Check {
        file: String,
        /// Show how long each line took
        #[arg(long)]
        time: bool,
    },
    /// Run the file, then one `measure` line without adding it, like `gcad measure case.gasm space at=0,0,5`
    Measure {
        file: String,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        what: Vec<String>,
    },
    /// List the faces and edges a selector matches after the whole file (or up to --line)
    Query {
        file: String,
        selector: String,
        #[arg(long)]
        line: Option<usize>,
    },
    /// Write the solid as .step, .stl, .obj, .3mf or an .svg drawing, or a .gdwg drawing file as .pdf or .svg sheets
    Export {
        file: String,
        output: String,
        /// For an .svg drawing, add a section view cut across this plane, like y=0
        #[arg(long)]
        section: Option<gcad::drawing::Section>,
    },
    /// Run a .gsim simulation, write NAME.result.bin beside it and print the readings
    Sim {
        file: String,
        /// Build the problem and print its size without solving it
        #[arg(long)]
        dry_run: bool,
        /// Print no progress while it runs
        #[arg(long, short)]
        quiet: bool,
        /// Build the parts here and solve them on a paid remote worker (see `gcad remote`)
        #[arg(long, conflicts_with = "dry_run")]
        remote: bool,
        /// The longest a remote run may take; its cost is held from the balance while it runs
        #[arg(long, default_value_t = 60, requires = "remote")]
        max_minutes: u64,
        /// The most a remote runner may charge, in euro an hour; 0 takes only free runners (default: any runner online)
        #[arg(long, requires = "remote", value_parser = parse_price)]
        max_price: Option<u64>,
    },
    /// Manage the account that pays for `gcad sim --remote`
    Remote {
        #[command(subcommand)]
        action: RemoteAction,
    },
    /// Measure the GPU the flow solver uses: FP32 TFLOPS and memory bandwidth, as JSON
    #[command(hide = true)]
    GpuBench,
    /// Solve a job packed by `gcad sim --remote` (the remote worker runs this)
    #[command(hide = true)]
    SimJob {
        job: String,
        out: String,
        #[arg(long, short)]
        quiet: bool,
    },
    /// List the materials a file can use: materials.toml files above it, ~/.config/gcad, and the built-in ones; or the materials in one library file
    Materials { file: Option<String> },
    /// List the parts an assembly is made of, counted by file, body and variables
    Bom {
        file: String,
        /// Write CSV instead of a table
        #[arg(long)]
        csv: bool,
    },
    /// Nest the flat parts of a file onto stock sheets and print where every piece and saw cut goes
    Nest {
        file: String,
        /// The stock sheet, length x width in mm; several sizes, comma separated, buy the mix that wastes least
        #[arg(long, default_value = "2440x1220")]
        stock: String,
        /// The stock's two long edges are finished (factory edge banded)
        #[arg(long)]
        edged: bool,
        /// Parts whose long edge shows and must sit on a finished stock edge (name patterns, repeatable)
        #[arg(long)]
        edge: Vec<String>,
        /// The least space between pieces: the saw kerf plus any allowance, in mm
        #[arg(long, default_value_t = 4.0)]
        gap: f64,
        /// Trim this much off every stock edge first, in mm
        #[arg(long, default_value_t = 0.0)]
        trim: f64,
        /// Only parts this thick (the thinnest side of each part's box)
        #[arg(long)]
        thickness: Option<f64>,
        /// Keep each part's length along the stock's length (grain or pattern)
        #[arg(long)]
        grain: bool,
        /// Parts to nest, by name with * wildcards; ! leaves parts out (default: all)
        parts: Vec<String>,
    },
    /// Draw iso, top, front and right views into one PNG
    Render {
        file: String,
        output: String,
        /// Draw an assembly with its parts moved apart as its `explode` lines say
        #[arg(long)]
        explode: bool,
        /// For a flow simulation, draw the air speed on slices through the parts
        #[arg(long)]
        flow: bool,
        /// For a flow simulation, draw the mean streamlines round the parts in 3D, coloured by speed
        #[arg(long)]
        lines: bool,
    },
    /// Print the format reference: every operation, selector and assembly line
    Docs,
    /// Open the file in a window that rebuilds whenever it is saved
    View {
        file: Option<String>,
        /// Highlight what this selector matches
        #[arg(long, default_value = "")]
        select: String,
        /// Show the model as it is after this line
        #[arg(long)]
        line: Option<usize>,
    },
}

#[derive(Subcommand)]
enum RemoteAction {
    /// Make an account and keep its token in ~/.config/gcad/remote.toml
    Signup {
        #[arg(long)]
        server: Option<String>,
    },
    /// Use an existing account token on this machine
    Login {
        token: String,
        #[arg(long)]
        server: Option<String>,
    },
    /// Show the account id and balance
    Account,
    /// List the runners: their GPU, measured speed and price an hour
    Runners,
    /// List recent remote runs
    Jobs,
    /// Stop a remote run; the time it used is still charged
    Cancel { id: String },
}

fn remote(action: RemoteAction) -> Result<()> {
    use gcad::remote;
    match action {
        RemoteAction::Signup { server } => remote::signup(server.as_deref()),
        RemoteAction::Login { token, server } => remote::login(&token, server.as_deref()),
        RemoteAction::Account => remote::account(),
        RemoteAction::Runners => remote::runners(),
        RemoteAction::Jobs => remote::jobs(),
        RemoteAction::Cancel { id } => remote::cancel(&id),
    }
}

fn simulate_remote(
    file: &str,
    vars: &[(String, f64)],
    limits: gcad::remote::Limits,
    quiet: bool,
) -> Result<()> {
    let path = std::path::Path::new(file);
    if !gcad::sim::is_sim(path) {
        bail!("`gcad sim` runs a .gsim file");
    }
    gcad::remote::simulate(path, vars, &limits, || {
        let _listening = (!quiet).then(progress_on_stderr);
        let run = {
            let _building = gcad::progress::stage("building the model", None, "");
            load(file, None, vars)?
        };
        finished(&run)?;
        Ok(run)
    })
}

fn load(file: &str, until: Option<usize>, vars: &[(String, f64)]) -> Result<Run> {
    run_path(std::path::Path::new(file), vars, until)
}

fn finished(run: &Run) -> Result<()> {
    if let Some((line, Err(error))) = run.steps.last() {
        bail!("line {}: {}: {error:#}", line.number, line.text);
    }
    if let Some(spec) = &run.model.sim {
        spec.target()?;
        return Ok(());
    }
    if run.model.solids().is_empty() {
        bail!("the file builds no solid");
    }
    Ok(())
}

fn check_drawing(file: &str, vars: &[(String, f64)]) -> Result<()> {
    let drawing = gcad::drafting::run(std::path::Path::new(file), vars)?;
    let width = drawing
        .steps
        .iter()
        .map(|(line, _)| line.text.len())
        .max()
        .unwrap_or(0);
    for (line, result) in &drawing.steps {
        match result {
            Ok(text) => println!("{:4} {:width$}  ok  {text}", line.number, line.text),
            Err(error) => println!("{:4} {:width$}  ERROR  {error:#}", line.number, line.text),
        }
    }
    if let Some((line, error)) = drawing.failed() {
        bail!("stopped at line {}: {error:#}", line.number);
    }
    println!("{} sheets", drawing.sheets.len());
    Ok(())
}

fn check(file: &str, vars: &[(String, f64)], time: bool) -> Result<()> {
    let run = load(file, None, vars)?;
    let width = run
        .steps
        .iter()
        .map(|(line, _)| line.text.len())
        .max()
        .unwrap_or(0);
    for (k, (line, result)) in run.steps.iter().enumerate() {
        let spent = match (time, run.took.get(k)) {
            (true, Some(d)) => format!("{:>7.0} ms ", d.as_secs_f64() * 1000.0),
            _ => String::new(),
        };
        match result {
            Ok(summary) => println!(
                "{:>4} {:<width$}  {spent}ok  {summary}",
                line.number, line.text
            ),
            Err(error) => {
                println!(
                    "{:>4} {:<width$}  {spent}ERROR  {error:#}",
                    line.number, line.text
                )
            }
        }
    }
    if time {
        let total: std::time::Duration = run.took.iter().sum();
        println!("{:.2} s in all", total.as_secs_f64());
    }
    let bodies: Vec<String> = run
        .model
        .body_names()
        .into_iter()
        .filter(|name| !run.model.members.contains(name))
        .collect();
    if !run.model.assembly && bodies.len() > 1 {
        println!(
            "note: {} separate bodies ({}). If they are separate parts, bring this file into a .gasm with `part` to place, join and check them; if they are one part, `combine` them",
            bodies.len(),
            bodies.join(", ")
        );
    }
    match run.steps.last() {
        Some((line, Err(_))) => bail!("stopped at line {}", line.number),
        _ => finished(&run)?,
    }
    if run.model.sim.is_some() {
        let path = std::path::Path::new(file);
        match gcad::sim::stale(path, vars) {
            Some(why) => println!("note: {why}"),
            None => {
                let result = gcad::sim::result_path(path);
                println!("{} is up to date", result.display());
                if let Ok(outcome) = gcad::sim::Outcome::read(&result) {
                    print!("{}", outcome.table());
                }
            }
        }
    }
    Ok(())
}

fn progress_on_stderr() -> gcad::progress::Listening {
    use std::io::{IsTerminal, Write};
    struct Seen {
        printed: std::collections::HashMap<String, std::time::Duration>,
    }
    let terminal = std::io::stderr().is_terminal();
    let seen = std::sync::Mutex::new(Seen {
        printed: Default::default(),
    });
    gcad::progress::listen(move |report| {
        let mut seen = seen.lock().unwrap_or_else(|e| e.into_inner());
        let mut err = std::io::stderr().lock();
        let line = match (&report.parent, terminal) {
            (Some(parent), true) => format!("{parent}: {}", report.line()),
            (Some(_), false) => format!("  {}", report.line()),
            (None, _) => report.line(),
        };
        if terminal {
            let shown: String = line.chars().take(200).collect();
            if report.finished && report.elapsed < std::time::Duration::from_millis(50) {
                let _ = write!(err, "\r\x1b[2K");
            } else if report.finished {
                let _ = writeln!(err, "\r\x1b[2K{shown}");
            } else {
                let _ = write!(err, "\r\x1b[2K{shown}");
            }
            let _ = err.flush();
            return;
        }
        let key = format!("{:?}/{}", report.phase, report.stage);
        let every = std::time::Duration::from_secs(10);
        let last = seen.printed.get(&key).copied().unwrap_or_default();
        if report.finished {
            seen.printed.remove(&key);
            if report.elapsed >= std::time::Duration::from_millis(50) {
                let _ = writeln!(err, "{line}");
            }
        } else if report.elapsed >= last + every {
            let _ = writeln!(err, "{line}");
            seen.printed.insert(key, report.elapsed);
        }
    })
}

fn simulate(file: &str, vars: &[(String, f64)], dry_run: bool) -> Result<()> {
    let path = std::path::Path::new(file);
    if !gcad::sim::is_sim(path) {
        bail!("`gcad sim` runs a .gsim file");
    }
    let run = {
        let _building = gcad::progress::stage("building the model", None, "");
        load(file, None, vars)?
    };
    finished(&run)?;
    let spec = run
        .model
        .sim
        .as_deref()
        .expect("a .gsim file runs as a simulation");
    if dry_run {
        println!("{}", gcad::sim::plan(spec)?);
        return Ok(());
    }
    let outcome = gcad::sim::simulate(spec)?;
    let out = {
        let _writing = gcad::progress::stage("writing the result", None, "");
        gcad::sim::save(path, vars, &outcome)?
    };
    print!("{}", outcome.table());
    println!("wrote {}", out.display());
    let failed = outcome.failed();
    if !failed.is_empty() {
        bail!("expectation not met: {}", failed.join("; "));
    }
    Ok(())
}

fn measure(file: &str, what: &str, vars: &[(String, f64)]) -> Result<()> {
    let mut run = load(file, None, vars)?;
    finished(&run)?;
    let after = run.steps.last().map_or(0, |(line, _)| line.number) + 1;
    let line = gcad::parse::parse_line(after, &format!("measure {what}"))?;
    println!("{}", run.model.apply(&line)?);
    Ok(())
}

fn query(file: &str, selector: &str, until: Option<usize>, vars: &[(String, f64)]) -> Result<()> {
    let run = load(file, until, vars)?;
    finished(&run)?;
    let solid = run
        .model
        .solid
        .as_ref()
        .ok_or_else(|| anyhow!("`query` looks at a part file's current body"))?;
    let tolerance = run.model.tolerance();
    let faces = select::faces(solid);
    let edge_part = selector.contains('&') || selector.contains('|');
    if !edge_part {
        let indices = select::select_faces(selector, solid, &run.model.groups, tolerance)?;
        println!("{} face(s)", indices.len());
        for i in indices {
            let face = &faces[i];
            let kind = match face.oriented_surface() {
                monstertruck::modeling::Surface::Plane(plane) => {
                    let n = plane.normal();
                    format!("plane normal ({:.3}, {:.3}, {:.3})", n.x, n.y, n.z)
                }
                _ => "curved".to_string(),
            };
            println!("  face {i}: {kind}, {} edges", face.edge_iter().count());
        }
    }
    let edges = select::select_edges(selector, solid, &run.model.groups, tolerance)?;
    println!("{} edge(s)", edges.len());
    for edge in edges {
        let (a, b) = (edge.front().point(), edge.back().point());
        println!(
            "  ({:.3}, {:.3}, {:.3}) -> ({:.3}, {:.3}, {:.3})",
            a.x, a.y, a.z, b.x, b.y, b.z
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    gcad::model::store_parts_in(gcad::model::default_part_store());
    let vars = cli.set;
    let command = cli.command.unwrap_or(Command::View {
        file: cli.file,
        select: String::new(),
        line: None,
    });
    match command {
        #[cfg(feature = "game-assets")]
        Command::AssetFbxRecover { directory } => {
            let removed = gcad::asset::fbx_package::recover(&directory)?;
            println!("{}", serde_json::json!({"removed":removed}));
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetFbxExperimental { input, output, helper, timeout_seconds } => {
            let output = if output.is_absolute() { output } else { std::env::current_dir()?.join(output) };
            let published = gcad::asset::fbx_helper::convert_experimental(
                &helper, &input, &output, std::time::Duration::from_secs(timeout_seconds),
                &std::sync::atomic::AtomicBool::new(false),
            )?;
            println!("{}", serde_json::json!({"experimental":true,"package":published}));
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetFbxHelper { helper } => {
            let info = gcad::asset::fbx_helper::inspect(
                &helper, std::time::Duration::from_secs(10),
                &std::sync::atomic::AtomicBool::new(false),
            )?;
            println!("{}", serde_json::to_string_pretty(&info)?);
            Ok(())
        }
        #[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
        Command::AssetPaintGuide { file, node, output, primitive, resolution, padding } => {
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            gcad::asset::paint_guide::export(&asset, &node, primitive, resolution, padding, std::path::Path::new(&output))?;
            println!("Painting guide: {output}");
            Ok(())
        }
        #[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
        Command::AssetPaintApply { file, guide, painted, output } => {
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            gcad::asset::paint_guide::apply(&asset, std::path::Path::new(&guide), std::path::Path::new(&painted), std::path::Path::new(&output))?;
            println!("Padded paint texture: {output}");
            Ok(())
        }
        #[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
        Command::AssetNormalizeTextures { file, output, pixels_per_metre, max_dimension } => {
            let output=std::path::Path::new(&output);
            anyhow::ensure!(!output.exists(), "preserving existing output directory");
            let mut asset=gcad::asset::author::load(std::path::Path::new(&file),&vars)?;
            let (scene,report)=gcad::asset::texel_density::normalize(&asset.scene,gcad::asset::texel_density::Settings {pixels_per_metre,max_dimension})?;
            asset.scene=scene;
            let bytes=asset.encode_glb()?;
            let report=serde_json::to_vec_pretty(&report)?;
            std::fs::create_dir(output)?;
            std::fs::write(output.join("asset.glb"),bytes)?;
            std::fs::write(output.join("density.json"),report)?;
            println!("Normalized asset: {}",output.display());
            Ok(())
        }
        #[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
        Command::AssetSprites { file, recipe, output } => {
            let bytes = std::fs::read(&recipe)?;
            anyhow::ensure!(bytes.len() <= 1024 * 1024, "sprite recipe exceeds 1 MiB");
            let recipe: gcad::view::asset::sprites::Recipe = serde_json::from_slice(&bytes)?;
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            gcad::view::asset::sprites::install_headless()?;
            gcad::view::asset::sprites::export(&asset, &recipe, std::path::Path::new(&output), &std::sync::atomic::AtomicBool::new(false))?;
            println!("Sprite sheet exported: {output}");
            Ok(())
        }
        #[cfg(all(feature = "game-assets", not(target_arch = "wasm32")))]
        Command::AssetPreview {
            file,
            clip,
            time,
            snapshot,
        } => {
            anyhow::ensure!(
                file.to_ascii_lowercase().ends_with(".gasset"),
                "asset-preview requires a .gasset file"
            );
            anyhow::ensure!(
                time.is_finite() && time >= 0.,
                "preview time must be finite and nonnegative"
            );
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            let clip = clip
                .map(|name| {
                    asset
                        .scene
                        .clips
                        .iter()
                        .position(|c| c.name == name)
                        .ok_or_else(|| anyhow!("unknown clip `{name}`"))
                })
                .transpose()?;
            if let Some(path) = snapshot {
                for difference in gcad::view::asset::differences(&asset) {
                    eprintln!("{difference}");
                }
                gcad::view::asset::snapshot(&asset, clip, time, std::path::Path::new(&path))
            } else {
                let clip = clip.map(|i| asset.scene.clips[i].name.clone());
                gcad::view::run_asset(std::path::PathBuf::from(file), vars, clip, time)
                    .map_err(|e| anyhow!("gcad viewer failed: {e}"))
            }
        }
        #[cfg(feature = "game-assets")]
        Command::AssetBakeNormals {
            file,
            node,
            source,
            source_node,
            output,
            primitive,
            source_primitive,
            width,
            height,
            max_distance,
            min_normal_dot,
            max_missed_fraction,
            padding,
        } => {
            use std::io::Write;
            anyhow::ensure!(
                file.to_ascii_lowercase().ends_with(".gasset")
                    && source.to_ascii_lowercase().ends_with(".gasset")
                    && output.to_ascii_lowercase().ends_with(".glb"),
                "asset-bake-normals requires two .gasset inputs and a new .glb output"
            );
            let evaluation_started = std::time::Instant::now();
            let mut target = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            let source = gcad::asset::author::load(std::path::Path::new(&source), &vars)?;
            let evaluation = evaluation_started.elapsed();
            let processing_started = std::time::Instant::now();
            let target_node = target
                .scene
                .nodes
                .iter()
                .position(|n| n.name == node)
                .ok_or_else(|| anyhow::anyhow!("normal bake target node not found"))?;
            let source_node = source
                .scene
                .nodes
                .iter()
                .position(|n| n.name == source_node)
                .ok_or_else(|| anyhow::anyhow!("normal bake source node not found"))?;
            let report = gcad::asset::projection::apply_to_scene(
                &mut target.scene,
                target_node,
                primitive,
                &source.scene,
                source_node,
                source_primitive,
                gcad::asset::projection::Options {
                    width,
                    height,
                    max_distance_mm: max_distance,
                    min_normal_dot,
                    max_missed_fraction,
                },
                padding,
            )?;
            let bytes = target.encode_glb()?;
            let processing = processing_started.elapsed();
            let writing_started = std::time::Instant::now();
            let mut destination = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            destination.write_all(&bytes)?;
            let timing = serde_json::json!({
                "evaluation": evaluation.as_secs_f64(),
                "derive_and_encode": processing.as_secs_f64(),
                "write": writing_started.elapsed().as_secs_f64(),
                "scope": "Wall-clock evaluation, derivation/validation/GLB encoding, and output open/write. Excludes diagnostic JSON serialization/output and engine work. Write is not an fsync durability measurement. Cache-dependent."
            });
            let mut diagnostic = serde_json::to_value(&report)?;
            diagnostic["processing_seconds"] = timing;
            println!("{}", serde_json::to_string_pretty(&diagnostic)?);
            eprintln!("wrote {output}");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetLod {
            file,
            node,
            output,
            primitive,
            triangles,
            max_error,
            max_uv_error,
            max_surface_error,
            max_surface_p95,
            max_silhouette_difference,
            max_angle,
            samples,
            max_trials,
            lock,
            bake_normals,
            bake_distance,
            bake_padding,
        } => {
            use std::io::Write;
            anyhow::ensure!(
                file.to_ascii_lowercase().ends_with(".gasset")
                    && output.to_ascii_lowercase().ends_with(".glb"),
                "asset-lod requires .gasset input and a new .glb output"
            );
            let limits = gcad::asset::lod::Acceptance {
                max_surface_mm: max_surface_error,
                max_surface_p95_mm: max_surface_p95,
                max_silhouette_difference,
            };
            limits.validate()?;
            let evaluation_started = std::time::Instant::now();
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            let evaluation = evaluation_started.elapsed();
            let processing_started = std::time::Instant::now();
            let node = asset
                .scene
                .nodes
                .iter()
                .position(|n| n.name == node)
                .ok_or_else(|| anyhow::anyhow!("unknown LOD node"))?;
            let (mut scene, skins, mut report) = gcad::asset::lod::derive_node(
                &asset.scene,
                &asset.skins,
                node,
                primitive,
                &lock,
                samples,
                &gcad::asset::lod::Options {
                    target_triangles: triangles,
                    max_error_mm: max_error,
                    max_uv_error,
                    max_angle_degrees: max_angle,
                    max_trials,
                },
            )?;
            report.accept(limits)?;
            if let Some(size) = bake_normals {
                report.normal_bake = Some(gcad::asset::projection::apply_to_scene(
                    &mut scene,
                    node,
                    primitive,
                    &asset.scene,
                    node,
                    primitive,
                    gcad::asset::projection::Options {
                        width: size,
                        height: size,
                        max_distance_mm: bake_distance.unwrap(),
                        min_normal_dot: 0.,
                        max_missed_fraction: 0.,
                    },
                    bake_padding,
                )?);
            }
            let bytes = gcad::asset::glb::encode_skinned_scene(&scene, &skins)?;
            let processing = processing_started.elapsed();
            let writing_started = std::time::Instant::now();
            let mut destination = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            destination.write_all(&bytes)?;
            let timing = serde_json::json!({
                "evaluation": evaluation.as_secs_f64(),
                "derive_and_encode": processing.as_secs_f64(),
                "write": writing_started.elapsed().as_secs_f64(),
                "scope": "Wall-clock evaluation, derivation/validation/GLB encoding, and output open/write. Excludes diagnostic JSON serialization/output and engine work. Write is not an fsync durability measurement. Cache-dependent."
            });
            let mut diagnostic = serde_json::to_value(&report)?;
            diagnostic["processing_seconds"] = timing;
            println!("{}", serde_json::to_string_pretty(&diagnostic)?);
            eprintln!("wrote {output}");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetRootMotion {
            file,
            node,
            output,
            clip,
            in_place,
            axes,
            rigid,
        } => {
            use std::io::Write;
            anyhow::ensure!(
                file.to_ascii_lowercase().ends_with(".gasset")
                    && output.to_ascii_lowercase().ends_with(".glb"),
                "asset-root-motion requires .gasset input and a new .glb output"
            );
            let mut selected = [false; 3];
            if let Some(axes) = axes {
                anyhow::ensure!(
                    !axes.is_empty(),
                    "root-motion axes must be a nonempty subset of xyz"
                );
                for axis in axes.chars() {
                    let index = match axis {
                        'x' => 0,
                        'y' => 1,
                        'z' => 2,
                        _ => anyhow::bail!("root-motion axes must be a subset of xyz"),
                    };
                    anyhow::ensure!(!selected[index], "duplicate root-motion axis");
                    selected[index] = true;
                }
            }
            let evaluation_started = std::time::Instant::now();
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            let evaluation = evaluation_started.elapsed();
            let processing_started = std::time::Instant::now();
            let node = asset
                .scene
                .nodes
                .iter()
                .position(|n| n.name == node)
                .ok_or_else(|| anyhow::anyhow!("root-motion node not found"))?;
            let clip = asset
                .scene
                .clips
                .iter()
                .position(|c| c.name == clip)
                .ok_or_else(|| anyhow::anyhow!("root-motion clip not found"))?;
            let (bytes, report) = if rigid {
                let (bytes, motion) = gcad::asset::glb::encode_rigid_root_motion(
                    &asset.scene,
                    &asset.skins,
                    clip,
                    node,
                    &in_place,
                )?;
                (bytes, motion.to_json()?)
            } else {
                let (bytes, motion) = gcad::asset::glb::encode_root_motion(
                    &asset.scene,
                    &asset.skins,
                    clip,
                    node,
                    selected,
                    &in_place,
                )?;
                (bytes, motion.to_json()?)
            };
            let processing = processing_started.elapsed();
            let writing_started = std::time::Instant::now();
            let mut destination = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            destination.write_all(&bytes)?;
            let timing = serde_json::json!({
                "evaluation": evaluation.as_secs_f64(),
                "derive_and_encode": processing.as_secs_f64(),
                "write": writing_started.elapsed().as_secs_f64(),
                "scope": "Wall-clock evaluation, derivation/validation/GLB encoding, and output open/write. Includes trajectory JSON serialization; excludes diagnostic output and engine work. Write is not an fsync durability measurement. Cache-dependent."
            });
            println!("{report}");
            eprintln!("{}", serde_json::json!({"processing_seconds": timing}));
            eprintln!("wrote {output}");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetReduce {
            file,
            output,
            clip,
            max_error,
            max_angle,
            subdivisions,
            max_trials,
        } => {
            use std::io::Write;
            anyhow::ensure!(
                file.to_ascii_lowercase().ends_with(".gasset")
                    && output.to_ascii_lowercase().ends_with(".glb"),
                "asset-reduce requires .gasset input and a new .glb output"
            );
            let evaluation_started = std::time::Instant::now();
            let mut asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            let evaluation = evaluation_started.elapsed();
            let processing_started = std::time::Instant::now();
            let index = asset
                .scene
                .clips
                .iter()
                .position(|c| c.name == clip)
                .ok_or_else(|| anyhow::anyhow!("unknown clip `{clip}`"))?;
            let (reduced, report) = gcad::asset::reduce::reduce(
                &asset.scene,
                &asset.skins,
                &asset.scene.clips[index],
                &gcad::asset::reduce::Options {
                    max_error_mm: max_error,
                    max_angle_degrees: max_angle,
                    subdivisions,
                    max_trials,
                },
            )?;
            asset.scene.clips[index] = reduced;
            let bytes = asset.encode_glb()?;
            let processing = processing_started.elapsed();
            let writing_started = std::time::Instant::now();
            let mut destination = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            destination.write_all(&bytes)?;
            let timing = serde_json::json!({
                "evaluation": evaluation.as_secs_f64(),
                "derive_and_encode": processing.as_secs_f64(),
                "write": writing_started.elapsed().as_secs_f64(),
                "scope": "Wall-clock evaluation, derivation/validation/GLB encoding, and output open/write. Excludes diagnostic JSON serialization/output and engine work. Write is not an fsync durability measurement. Cache-dependent."
            });
            let mut diagnostic = serde_json::to_value(&report)?;
            diagnostic["processing_seconds"] = timing;
            println!("{}", serde_json::to_string_pretty(&diagnostic)?);
            eprintln!("wrote {output}");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetQuantizeWeights { file, output, max_error, max_angle, samples } => {
            use std::io::Write;
            anyhow::ensure!(file.to_ascii_lowercase().ends_with(".gasset")
                && output.to_ascii_lowercase().ends_with(".glb"),
                "asset-quantize-weights requires .gasset input and a new .glb output");
            let started = std::time::Instant::now();
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            let evaluated = started.elapsed();
            let (bytes, report) = gcad::asset::quantize::encode(&asset.scene, &asset.skins,
                &gcad::asset::quantize::Options { samples_per_clip: samples,
                    max_error_mm: max_error, max_angle_degrees: max_angle })?;
            let processing = started.elapsed().saturating_sub(evaluated);
            let writing = std::time::Instant::now();
            let mut destination = std::fs::OpenOptions::new().write(true).create_new(true).open(&output)?;
            destination.write_all(&bytes)?;
            let elapsed_write = writing.elapsed();
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "version": 1, "encoding": "UNORM16", "validation": report,
                "limits": {"max_error_mm": max_error, "max_angle_degrees": max_angle},
                "encoded_bytes": bytes.len(),
                "scope": "Sampled weight-only error; excludes other float32 encoding and engine errors. Not a continuous bound.",
                "processing_seconds": {"evaluation": evaluated.as_secs_f64(),
                    "derive_and_encode": processing.as_secs_f64(), "write": elapsed_write.as_secs_f64(),
                    "scope": "Wall-clock evaluation, quantisation/validation/GLB encoding and buffered open/write; excludes report output, fsync durability and engine work."}
            }))?);
            eprintln!("wrote {output}");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetPruneJoints { file, output, keep } => {
            use std::io::Write;
            anyhow::ensure!(file.to_ascii_lowercase().ends_with(".gasset")
                && output.to_ascii_lowercase().ends_with(".glb"),
                "asset-prune-joints requires .gasset input and a new .glb output");
            let started = std::time::Instant::now();
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            anyhow::ensure!(!asset.skins.is_empty(), "asset has no skins to prune");
            let keep = keep.iter().map(|name| asset.scene.nodes.iter().position(|node| &node.name == name)
                .ok_or_else(|| anyhow!("protected node `{name}` does not exist"))).collect::<Result<Vec<_>>>()?;
            let evaluated = started.elapsed();
            let mut skins = Vec::new();
            let mut remaps = Vec::new();
            for skin in &asset.skins {
                let (derived, report) = skin.prune_palette(&asset.scene, &keep)?;
                skins.push(derived);
                remaps.push(report);
            }
            let bytes = gcad::asset::glb::encode_skinned_scene(&asset.scene, &skins)?;
            let processing = started.elapsed().saturating_sub(evaluated);
            let writing = std::time::Instant::now();
            let mut destination = std::fs::OpenOptions::new().write(true).create_new(true).open(&output)?;
            destination.write_all(&bytes)?;
            let elapsed_write = writing.elapsed();
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "version": 1, "scene_nodes_preserved": true, "skins": remaps,
                "encoded_bytes": bytes.len(),
                "processing_seconds": {"evaluation": evaluated.as_secs_f64(),
                    "derive_and_encode": processing.as_secs_f64(), "write": elapsed_write.as_secs_f64(),
                    "scope": "Wall-clock evaluation, palette derivation/GLB encoding and buffered open/write; excludes report output and engine work."}
            }))?);
            eprintln!("wrote {output}");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetFaceBind { file, source, selector, output, body } => {
            use std::io::Write;
            anyhow::ensure!(file.to_ascii_lowercase().ends_with(".gasset")
                && output.to_ascii_lowercase().ends_with(".json"),
                "asset-face-bind requires .gasset input and a new .json output");
            let source = gcad::asset::face_author::load(std::path::Path::new(&file), &vars, &source, body.as_deref())?;
            let record = source.capture(&selector)?;
            let bytes = record.to_json()?;
            let mut destination = std::fs::OpenOptions::new().write(true).create_new(true).open(&output)?;
            destination.write_all(&bytes)?;
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "version": 1, "selected_faces": record.selected_face_count(), "record_bytes": bytes.len(),
                "scope": "Source-bound raw tessellation; no render-mesh preparation or automatic rebinding."
            }))?);
            eprintln!("wrote {output}");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetFaceCheck { file, source, selection, body } => {
            anyhow::ensure!(file.to_ascii_lowercase().ends_with(".gasset"), "asset-face-check requires .gasset input");
            let record = gcad::asset::faces::SelectionFile::from_json(&gcad::host::read(std::path::Path::new(&selection))?)?;
            let source = gcad::asset::face_author::load(std::path::Path::new(&file), &vars, &source, body.as_deref())?;
            let resolved = source.resolve(&record)?;
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "version": 1, "current": true, "selected_faces": record.selected_face_count(),
                "selected_triangles": resolved.triangles.len(), "total_triangles": resolved.mesh.triangles.len(),
                "scope": "Current raw CAD tessellation in source-body local millimetres; excludes render-mesh preparation."
            }))?);
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetFaceWeights { file, node, selection, output, influence, boundary } => {
            anyhow::ensure!(file.to_ascii_lowercase().ends_with(".gasset") && output.to_ascii_lowercase().ends_with(".glb"), "asset-face-weights requires .gasset input and new .glb output");
            let record = gcad::asset::faces::SelectionFile::from_json(&gcad::host::read(std::path::Path::new(&selection))?)?;
            let asset = gcad::asset::author::load_with_faces(std::path::Path::new(&file), &vars)?;
            let index = asset.scene.nodes.iter().position(|n| n.name == node).ok_or_else(|| anyhow!("face target node does not exist"))?;
            let policy = match boundary.as_str() {
                "split" => gcad::asset::face_weights::Boundary::Split,
                "reject-shared" => gcad::asset::face_weights::Boundary::RejectShared,
                _ => bail!("unknown face weight boundary policy"),
            };
            let influences: Vec<_> = influence.iter().map(|(n,w)| (n.as_str(),*w)).collect();
            let edited = gcad::asset::face_weights::edit_scene(&asset,index,&record,&influences,policy)?;
            let bytes = edited.encode_glb()?;
            use std::io::Write;
            let mut destination = std::fs::OpenOptions::new().write(true).create_new(true).open(&output)?;
            destination.write_all(&bytes)?;
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "version": 1, "node": node, "selected_triangles": edited.selected_triangles,
                "assigned_vertices": edited.assigned_vertices.iter().flatten().filter(|v| **v).count(),
                "split_vertices": edited.split_vertices, "boundary": boundary, "encoded_bytes": bytes.len(),
                "scope": "Selected corner weights replaced; unselected weights preserved. Separate tuples can diverge during deformation, including existing shading seams."
            }))?);
            eprintln!("wrote {output}");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetFaceUv { file, node, selection, output, scale, rotation, offset } => {
            anyhow::ensure!(file.to_ascii_lowercase().ends_with(".gasset") && output.to_ascii_lowercase().ends_with(".glb"), "asset-face-uv requires .gasset input and new .glb output");
            let pair = |text: &str| -> Result<[f64;2]> {
                let values = text.split(',').map(str::parse::<f64>).collect::<std::result::Result<Vec<_>,_>>()?;
                values.try_into().map_err(|_| anyhow!("UV scale/offset requires two comma-separated values"))
            };
            let transform = gcad::asset::uv_edit::Transform { scale: pair(&scale)?, rotation_degrees: rotation, offset: pair(&offset)? };
            transform.validate()?;
            let record = gcad::asset::faces::SelectionFile::from_json(&gcad::host::read(std::path::Path::new(&selection))?)?;
            let asset = gcad::asset::author::load_with_faces(std::path::Path::new(&file), &vars)?;
            let index = asset.scene.nodes.iter().position(|n| n.name == node).ok_or_else(|| anyhow!("face target node does not exist"))?;
            let edited = gcad::asset::uv_edit::edit_scene(&asset, index, &record, transform)?;
            let bytes = edited.encode_glb()?;
            use std::io::Write;
            let mut destination = std::fs::OpenOptions::new().write(true).create_new(true).open(&output)?;
            destination.write_all(&bytes)?;
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "version": 1, "node": node, "selected_triangles": edited.selected_triangles,
                "encoded_bytes": bytes.len(), "scale": transform.scale, "rotation_degrees": rotation, "offset": transform.offset,
                "scope": "Selected UV sampling coordinates transformed; materials and texture pixels unchanged; skin weights remapped exactly."
            }))?);
            eprintln!("wrote {output}");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetFaceMaterial { file, node, selection, output, surface } => {
            anyhow::ensure!(file.to_ascii_lowercase().ends_with(".gasset") && output.to_ascii_lowercase().ends_with(".glb"), "asset-face-material requires .gasset input and new .glb output");
            let mut named = std::collections::BTreeMap::new();
            for setting in surface {
                let (key, value) = setting.split_once('=').ok_or_else(|| anyhow!("surface setting requires KEY=VALUE"))?;
                anyhow::ensure!(named.insert(key.to_string(), value.to_string()).is_none(), "duplicate surface setting");
            }
            let settings = gcad::asset::author::SurfaceSettings::parse(&named)?;
            let record = gcad::asset::faces::SelectionFile::from_json(&gcad::host::read(std::path::Path::new(&selection))?)?;
            let asset = gcad::asset::author::load_with_faces(std::path::Path::new(&file), &vars)?;
            let index = asset.scene.nodes.iter().position(|n| n.name == node).ok_or_else(|| anyhow!("face target node does not exist"))?;
            let derived = gcad::asset::face_material::assign(&asset, index, &record, &settings)?;
            let bytes = derived.encode_glb()?;
            use std::io::Write;
            let mut destination = std::fs::OpenOptions::new().write(true).create_new(true).open(&output)?;
            destination.write_all(&bytes)?;
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "version": 1, "node": node, "selected_triangles": derived.selected_triangles,
                "primitives": derived.source_triangles.len(), "encoded_bytes": bytes.len(),
                "scope": "Derived material primitives; source asset, selection, vertex attributes and weight values preserved."
            }))?);
            eprintln!("wrote {output}");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetFaceResolve { file, node, selection } => {
            anyhow::ensure!(file.to_ascii_lowercase().ends_with(".gasset"), "asset-face-resolve requires .gasset input");
            let record = gcad::asset::faces::SelectionFile::from_json(&gcad::host::read(std::path::Path::new(&selection))?)?;
            let asset = gcad::asset::author::load_with_faces(std::path::Path::new(&file), &vars)?;
            let index = asset.scene.nodes.iter().position(|n| n.name == node).ok_or_else(|| anyhow!("face target node does not exist"))?;
            let selections = asset.resolve_face_primitives(index, &record)?;
            let primitives = &asset.scene.meshes[asset.scene.nodes[index].mesh.unwrap()].primitives;
            if selections.len() > 1 {
                let results: Vec<_> = selections.iter().zip(primitives).enumerate().map(|(i,(triangles, primitive))| serde_json::json!({
                    "primitive": i, "triangles": triangles, "selected_triangles": triangles.len(), "total_triangles": primitive.geometry.triangles.len()
                })).collect();
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "version": 2, "current": true, "node": node, "selected_faces": record.selected_face_count(),
                    "selected_triangles": selections.iter().map(Vec::len).sum::<usize>(), "primitives": results,
                    "scope": "Prepared node-local mesh from this evaluation; triangle indices are local to each primitive and transient."
                }))?);
                return Ok(());
            }
            let triangles = &selections[0];
            let mesh = &primitives[0].geometry;
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "version": 1, "current": true, "node": node, "primitive": 0,
                "selected_faces": record.selected_face_count(), "selected_triangles": triangles.len(),
                "total_triangles": mesh.triangles.len(), "triangles": triangles,
                "scope": "Prepared node-local mesh from this evaluation; indices are transient. Source record remains unchanged."
            }))?);
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetCapabilities { file } => {
            anyhow::ensure!(file.to_ascii_lowercase().ends_with(".gasset"),
                "asset-capabilities requires a .gasset file");
            let started = std::time::Instant::now();
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            let evaluated = started.elapsed();
            let encoding = std::time::Instant::now();
            let report = gcad::asset::capabilities::inspect(&asset.scene, &asset.skins);
            let inspected = encoding.elapsed();
            let mut output = serde_json::to_value(&report)?;
            output["processing_seconds"] = serde_json::json!({
                "evaluation": evaluated.as_secs_f64(),
                "encode_and_inspect": inspected.as_secs_f64(),
                "scope": "Wall-clock evaluation and in-memory GLB encoding/inspection; excludes report serialization, output I/O and engine work. Cache-dependent."
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            anyhow::ensure!(report.exportable, "asset cannot be encoded as GLB; see capability report");
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetReport {
            file,
            samples,
            shading,
        } => {
            anyhow::ensure!(
                file.to_ascii_lowercase().ends_with(".gasset"),
                "asset-report requires a .gasset file"
            );
            anyhow::ensure!(
                (2..=10001).contains(&samples),
                "samples must be between 2 and 10001"
            );
            let start = std::time::Instant::now();
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            let loaded = start.elapsed();
            let report = gcad::asset::report::inspect_with_shading(
                &asset.scene,
                &asset.skins,
                samples,
                shading,
            )?;
            let inspected = start.elapsed().saturating_sub(loaded);
            let mut output = serde_json::to_value(&report)?;
            output["processing_seconds"] = serde_json::json!({
                "evaluation": loaded.as_secs_f64(),
                "diagnostics": inspected.as_secs_f64(),
                "scope": "Wall-clock evaluation and sampled diagnostics; excludes report serialization, output I/O and engine work. Cache-dependent."
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            eprintln!(
                "evaluation: {:.3}s; report: {:.3}s",
                loaded.as_secs_f64(),
                inspected.as_secs_f64()
            );
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::AssetBind {
            file,
            node,
            output,
            joints,
            blend,
            chain,
            deferred_skins,
        } => {
            use gcad::asset::skin::{Assignment, Skin};
            use std::io::Write;
            anyhow::ensure!(
                file.to_ascii_lowercase().ends_with(".gasset"),
                "asset-bind requires a .gasset file"
            );
            let asset =
                gcad::asset::author::load_for_binding_deferred(std::path::Path::new(&file), &vars, &node, &deferred_skins)?;
            let node_index = asset
                .scene
                .nodes
                .iter()
                .position(|n| n.name == node)
                .ok_or_else(|| anyhow!("unknown mesh node"))?;
            let joint_indices = joints
                .iter()
                .map(|name| {
                    asset
                        .scene
                        .nodes
                        .iter()
                        .position(|n| &n.name == name)
                        .ok_or_else(|| anyhow!("unknown joint `{name}`"))
                })
                .collect::<Result<Vec<_>>>()?;
            let assignment = if let Some(blend) = blend {
                let parts: Vec<_> = blend.split(',').collect();
                anyhow::ensure!(parts.len() == 3, "blend expects AXIS,START,END");
                let axis = match parts[0] {
                    "x" => 0,
                    "y" => 1,
                    "z" => 2,
                    _ => bail!("blend axis must be x, y or z"),
                };
                Assignment::Linear {
                    axis,
                    start_mm: parts[1].parse()?,
                    end_mm: parts[2].parse()?,
                }
            } else {
                Assignment::Rigid
            };
            let skin = if let Some(chain) = chain {
                let parts: Vec<_> = chain.split(',').collect();
                anyhow::ensure!(parts.len() >= 3, "chain expects AXIS,KNOT0,KNOT1,...");
                let axis = match parts[0] {
                    "x" => 0, "y" => 1, "z" => 2,
                    _ => bail!("chain axis must be x, y or z"),
                };
                let knots = parts[1..].iter().map(|v| v.parse::<f64>()).collect::<std::result::Result<Vec<_>, _>>()?;
                Skin::assign_chain(&asset.scene, node_index, joint_indices, axis, &knots)?
            } else {
                Skin::assign(&asset.scene, node_index, joint_indices, assignment)?
            };
            let bytes = asset.weight_file(&skin)?.to_json()?;
            let mut destination = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?;
            destination.write_all(&bytes)?;
            println!(
                "wrote {output}; reference it with `skin {node} weights=PATH` relative to the asset file"
            );
            Ok(())
        }
        #[cfg(feature = "game-assets")]
        Command::Check { file, time } if file.to_ascii_lowercase().ends_with(".gasset") => {
            let start = std::time::Instant::now();
            let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
            println!(
                "asset valid: {} meshes, {} nodes, {} skins",
                asset.scene.meshes.len(),
                asset.scene.nodes.len(),
                asset.skins.len()
            );
            if time {
                println!("elapsed: {:.3}s", start.elapsed().as_secs_f64());
            }
            Ok(())
        }
        Command::Check { file, time: _ }
            if gcad::drafting::is_drawing(std::path::Path::new(&file)) =>
        {
            check_drawing(&file, &vars)
        }
        Command::Check { file, time } => check(&file, &vars, time),
        Command::Measure { file, what } => measure(&file, &what.join(" "), &vars),
        Command::Docs => {
            print!("{}", include_str!("../docs/format.md"));
            Ok(())
        }
        Command::Query {
            file,
            selector,
            line,
        } => query(&file, &selector, line, &vars),
        Command::Export {
            file,
            output,
            section,
        } => {
            #[cfg(feature = "game-assets")]
            if file.to_ascii_lowercase().ends_with(".gasset") {
                anyhow::ensure!(section.is_none(), "asset export does not support --section");
                anyhow::ensure!(
                    output.to_ascii_lowercase().ends_with(".glb"),
                    "asset export currently supports .glb only"
                );
                let asset = gcad::asset::author::load(std::path::Path::new(&file), &vars)?;
                let bytes = asset.encode_glb()?;
                std::fs::write(&output, bytes)?;
                println!("wrote {output}");
                return Ok(());
            }
            if gcad::drafting::is_drawing(std::path::Path::new(&file)) {
                let drawing = gcad::drafting::run(std::path::Path::new(&file), &vars)?;
                if let Some((line, error)) = drawing.failed() {
                    bail!("line {}: {}: {error:#}", line.number, line.text);
                }
                for written in gcad::drafting::write(&drawing, &output)? {
                    println!("wrote {written}");
                }
                return Ok(());
            }
            let run = load(&file, None, &vars)?;
            finished(&run)?;
            if let Some(section) = section {
                if !output.to_ascii_lowercase().ends_with(".svg") {
                    bail!("--section only applies to an .svg drawing");
                }
                let parts = run.model.parts();
                std::fs::write(
                    &output,
                    gcad::drawing::drawing_noted(&parts, Some(section), &run.model.hole_notes),
                )?;
                return Ok(());
            }
            export::export_model(&run.model, std::path::Path::new(&file), &output)
        }
        Command::Sim {
            file,
            quiet,
            remote: true,
            max_minutes,
            max_price,
            ..
        } => simulate_remote(
            &file,
            &vars,
            gcad::remote::Limits {
                minutes: max_minutes,
                price_cents: max_price,
            },
            quiet,
        ),
        Command::Remote { action } => remote(action),
        Command::GpuBench => {
            let bench = gcad::flow::bench::run(&gcad::flow::gpu::open()?)?;
            println!(
                "{}",
                serde_json::to_string(&gcad_sims_proto::GpuBench {
                    gpu: bench.gpu,
                    tflops: (bench.tflops * 10.0).round() / 10.0,
                    bandwidth_gbs: bench.bandwidth.round(),
                })?
            );
            Ok(())
        }
        Command::SimJob { job, out, quiet } => {
            let _listening = (!quiet).then(progress_on_stderr);
            gcad::remote::run_job(std::path::Path::new(&job), std::path::Path::new(&out))
        }
        Command::Sim {
            file,
            dry_run,
            quiet,
            ..
        } => {
            let _listening = (!quiet).then(progress_on_stderr);
            simulate(&file, &vars, dry_run)
        }
        Command::Materials { file } => {
            println!(
                "{}",
                gcad::model::materials::table(file.as_deref().map(std::path::Path::new))?
            );
            Ok(())
        }
        Command::Nest {
            file,
            stock,
            gap,
            trim,
            thickness,
            grain,
            parts,
            edged,
            edge,
        } => {
            let run = load(&file, None, &vars)?;
            finished(&run)?;
            let names = gcad::nest::pick(&run.model, &parts)?;
            let mut pieces = gcad::nest::panels(&run.model, &names, thickness)?;
            gcad::nest::mark_edges(&mut pieces, &edge)?;
            let layout = gcad::nest::nest(
                pieces,
                gcad::nest::Options {
                    stocks: gcad::nest::stock_sizes(&stock)?,
                    gap,
                    trim,
                    grain,
                    edged,
                },
            )?;
            print!("{}", layout.report());
            Ok(())
        }
        Command::Bom { file, csv } => {
            let run = load(&file, None, &vars)?;
            finished(&run)?;
            let items = gcad::bom::items(&run.model, std::path::Path::new(&file));
            if csv {
                print!("{}", gcad::bom::csv(&items));
            } else {
                println!("{}", gcad::bom::table(&items));
            }
            Ok(())
        }
        Command::Render {
            file,
            output,
            explode,
            flow,
            lines,
        } => {
            if gcad::sim::is_sim(std::path::Path::new(&file)) {
                let run = load(&file, None, &vars)?;
                finished(&run)?;
                let spec = run
                    .model
                    .sim
                    .as_deref()
                    .expect("a .gsim file runs as a simulation");
                if flow {
                    let result = gcad::sim::result_path(std::path::Path::new(&file));
                    let outcome = gcad::sim::Outcome::read(&result)?;
                    return gcad::plot::flow(&outcome.slices, &output);
                }
                let result = gcad::sim::result_path(std::path::Path::new(&file));
                if lines {
                    return gcad::sim::render_lines(spec, &result, &output);
                }
                return gcad::sim::render(spec, &result, &output);
            }
            let run = load(&file, None, &vars)?;
            finished(&run)?;
            let parts = run.model.exploded_parts(if explode { 1.0 } else { 0.0 });
            let parts: Vec<_> = parts.iter().map(|(s, c)| (s, *c)).collect();
            render::render_coloured(&parts, &output)
        }
        Command::View { file, select, line } => {
            gcad::view::run(file.map(Into::into), select, line, vars)
                .map_err(|error| anyhow!("{error}"))
        }
    }
}

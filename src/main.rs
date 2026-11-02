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
    if run.model.solids().is_empty() && run.model.meshes().is_empty() {
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
            let meshes = run.model.coloured_meshes();
            render::render_with_meshes(&parts, &meshes, &output)
        }
        Command::View { file, select, line } => {
            gcad::view::run(file.map(Into::into), select, line, vars)
                .map_err(|error| anyhow!("{error}"))
        }
    }
}

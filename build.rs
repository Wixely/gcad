fn git(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn build_version() {
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let code = [
        "log",
        "-1",
        "--format=%h",
        "--abbrev=12",
        "--",
        "src",
        "sims-proto",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
    ];
    let full = match git(&code) {
        Some(commit) => format!("{version}+{commit}"),
        None => version,
    };
    println!("cargo:rustc-env=GCAD_BUILD_VERSION={full}");
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
    }
    if let Some(common) = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"]) {
        println!("cargo:rerun-if-changed={common}/packed-refs");
        if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={common}/{branch}");
        }
    }
}

fn main() {
    build_version();
    // The expanded feature-enabled CLI can exceed the MSVC 1 MiB main-thread
    // reserve before parsing arguments in unoptimized builds. Reserve address
    // space (not committed memory) so the normal Debug launch remains usable.
    if std::env::var_os("CARGO_FEATURE_GAME_ASSETS").is_some()
        && std::env::var("PROFILE").as_deref() == Ok("debug")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg-bin=gcad=/STACK:8388608");
    }
    println!("cargo:rerun-if-changed=packaging/windows/icon.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("packaging/windows/icon.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon not embedded: {e}");
        }
    }
}

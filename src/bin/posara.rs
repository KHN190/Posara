use std::path::PathBuf;
use std::process::ExitCode;

use posara::{Host, runner};

fn usage() -> ExitCode {
    eprintln!("usage: posara <command> [flags] <cart.abe | cart.pk>");
    eprintln!();
    eprintln!("  run      run a cart in a window (or --headless)");
    eprintln!("  check    typecheck + lint, no run");
    eprintln!("  test     run every pub fn test_*()");
    eprintln!("  bench    headless N frames, no vsync — for profilers");
    eprintln!("  disasm   dump bytecode");
    eprintln!("  dump     grab a PNG at a time/frame");
    eprintln!("  record   render a WAV (+ optional PNG frames)");
    eprintln!("  build    transpile to a Rust crate (PK + native bridge) for cargo build");
    eprintln!();
    eprintln!("flags:");
    eprintln!("  common   --root <dir>");
    eprintln!("  build    --out <dir>");
    eprintln!("  run      --headless --mute --profile --trace --handlers --leak --debug");
    eprintln!("  bench    --frames N");
    eprintln!("  dump     --headless --out <png> [--at-ms T | --at-frame N] [--region x,y,w,h]");
    eprintln!("  record   --headless --out <wav> --duration <ms> [--frames <dir> --fps N --from <ms>]");
    eprintln!();
    eprintln!("env: BREAK_AT=<fn>:<pc>  dump the register window at that op");
    ExitCode::from(2)
}

fn parse_region(s: &str) -> Option<(i64, i64, i64, i64)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 { return None; }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?, parts[3].parse().ok()?))
}

struct Common {
    root: Option<PathBuf>,
    path: Option<String>,
    headless: bool,
    muted: bool,
}

fn parse_root(common: &mut Common, args: &mut std::iter::Skip<std::env::Args>, key: &str) -> Result<bool, ()> {
    match key {
        "--root" => match args.next() {
            Some(r) => { common.root = Some(PathBuf::from(r)); Ok(true) }
            None => Err(()),
        },
        _ => Ok(false),
    }
}

fn resolve_root(common: &Common) -> Option<(PathBuf, PathBuf)> {
    let path = PathBuf::from(common.path.as_ref()?);
    let root = common.root.clone().unwrap_or_else(|| runner::default_root(&path));
    eprintln!("• root: {}", root.display());
    Some((root, path))
}

fn cmd_run(mut args: std::iter::Skip<std::env::Args>) -> ExitCode {
    let mut common = Common { root: None, path: None, headless: false, muted: false };
    let mut profile = false;
    let mut dbg = runner::DebugCfg::default();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--profile" => profile = true,
            "--headless" => common.headless = true,
            "--mute" => common.muted = true,
            "--trace" => dbg.trace = true,
            "--handlers" => dbg.handlers = true,
            "--leak" => dbg.leak = true,
            "--debug" => { dbg.trace = true; dbg.handlers = true; }
            _ if parse_root(&mut common, &mut args, &a).unwrap_or(false) => {}
            _ => common.path = Some(a),
        }
    }
    let Some((root, path)) = resolve_root(&common) else { return usage(); };
    let host = match Host::new_cart(root.clone(), common.headless, common.muted, &path) {
        Ok(h) => h,
        Err(e) => { eprintln!("host init failed: {e}"); return ExitCode::from(1); }
    };
    let r = match runner::load_module(&path, &host) {
        Ok(r) => r,
        Err(e) => { eprintln!("{e}"); return ExitCode::from(1); }
    };
    for w in &r.warnings { eprintln!("{}", w.pretty_print()); }
    let (module, static_names, fn_names) = (r.module, r.static_names, r.fn_names);
    let reload = if path.extension().and_then(|s| s.to_str()) == Some("abe") {
        Some(path.clone())
    } else {
        None
    };
    match runner::run_module(module, static_names, fn_names, &host, reload, profile, dbg) {
        Ok(code) if code == 0 => ExitCode::SUCCESS,
        Ok(code) => ExitCode::from((code as i32).clamp(0, 255) as u8),
        Err(e) => { eprintln!("{e}"); ExitCode::from(1) }
    }
}

fn cmd_build(mut args: std::iter::Skip<std::env::Args>) -> ExitCode {
    let mut common = Common { root: None, path: None, headless: true, muted: false };
    let mut out: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => match args.next() { Some(v) => out = Some(PathBuf::from(v)), None => return usage() },
            _ if parse_root(&mut common, &mut args, &a).unwrap_or(false) => {}
            _ => common.path = Some(a),
        }
    }
    let Some((root, path)) = resolve_root(&common) else { return usage(); };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("cart");
    let out = out.unwrap_or_else(|| PathBuf::from(format!("{stem}-build-rs")));
    let host = match Host::new_cart(root.clone(), common.headless, common.muted, &path) {
        Ok(h) => h,
        Err(e) => { eprintln!("host init failed: {e}"); return ExitCode::from(1); }
    };
    match runner::build_crate(&path, &host, &out) {
        Ok(dir) => { eprintln!("• wrote crate {}\n  build: cargo build --release --manifest-path {}/Cargo.toml", dir.display(), dir.display()); ExitCode::SUCCESS }
        Err(e) => { eprintln!("{e}"); ExitCode::from(1) }
    }
}

fn cmd_check(mut args: std::iter::Skip<std::env::Args>) -> ExitCode {
    let mut common = Common { root: None, path: None, headless: true, muted: false };
    while let Some(a) = args.next() {
        match a.as_str() {
            _ if parse_root(&mut common, &mut args, &a).unwrap_or(false) => {}
            _ => common.path = Some(a),
        }
    }
    let Some((root, path)) = resolve_root(&common) else { return usage(); };
    let host = match Host::new_cart(root.clone(), common.headless, common.muted, &path) {
        Ok(h) => h,
        Err(e) => { eprintln!("host init failed: {e}"); return ExitCode::from(1); }
    };
    match runner::load_module(&path, &host) {
        Ok(r) => {
            for w in &r.warnings {
                eprintln!("{}", w.pretty_print());
            }
            eprintln!("ok");
            ExitCode::SUCCESS
        }
        Err(e) => { eprintln!("{e}"); ExitCode::from(1) }
    }
}

#[cfg(feature = "test")]
fn cmd_test(mut args: std::iter::Skip<std::env::Args>) -> ExitCode {
    let mut common = Common { root: None, path: None, headless: true, muted: false };
    while let Some(a) = args.next() {
        match a.as_str() {
            _ if parse_root(&mut common, &mut args, &a).unwrap_or(false) => {}
            _ => common.path = Some(a),
        }
    }
    let Some((root, path)) = resolve_root(&common) else { return usage(); };
    let host = match Host::new_cart(root.clone(), true, common.muted, &path) {
        Ok(h) => h,
        Err(e) => { eprintln!("host init failed: {e}"); return ExitCode::from(1); }
    };
    let r = match runner::load_module(&path, &host) {
        Ok(r) => r,
        Err(e) => { eprintln!("{e}"); return ExitCode::from(1); }
    };
    for w in &r.warnings { eprintln!("{}", w.pretty_print()); }
    match runner::run_tests(r.module, r.static_names, r.fn_names, &host) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => { eprintln!("{e}"); ExitCode::from(1) }
    }
}

fn cmd_dump(mut args: std::iter::Skip<std::env::Args>) -> ExitCode {
    let mut common = Common { root: None, path: None, headless: false, muted: false };
    let mut out: Option<PathBuf> = None;
    let mut at_ms: Option<u64> = None;
    let mut at_frame: Option<u64> = None;
    let mut region: Option<(i64, i64, i64, i64)> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => match args.next() { Some(v) => out = Some(PathBuf::from(v)), None => return usage() },
            "--at-ms" => match args.next().and_then(|v| v.parse().ok()) { Some(v) => at_ms = Some(v), None => return usage() },
            "--at-frame" => match args.next().and_then(|v| v.parse().ok()) { Some(v) => at_frame = Some(v), None => return usage() },
            "--region" => match args.next().as_deref().and_then(parse_region) { Some(v) => region = Some(v), None => return usage() },
            "--headless" => common.headless = true,
            _ if parse_root(&mut common, &mut args, &a).unwrap_or(false) => {}
            _ => common.path = Some(a),
        }
    }
    let Some(out) = out else { return usage(); };
    let Some((root, path)) = resolve_root(&common) else { return usage(); };
    let host = match Host::new_cart(root.clone(), common.headless, common.muted, &path) {
        Ok(h) => h,
        Err(e) => { eprintln!("host init failed: {e}"); return ExitCode::from(1); }
    };
    let r = match runner::load_module(&path, &host) {
        Ok(r) => r,
        Err(e) => { eprintln!("{e}"); return ExitCode::from(1); }
    };
    let run_result = if let Some(n) = at_frame {
        runner::run_until_frame(r.module, r.static_names, r.fn_names, &host, n)
    } else {
        runner::run_until_ms(r.module, r.static_names, r.fn_names, &host, at_ms.unwrap_or(0))
    };
    if let Err(e) = run_result {
        eprintln!("{e}");
        return ExitCode::from(1);
    }
    let fb = host.gfx.fb.borrow();
    let (x, y, w, h) = region.unwrap_or((0, 0, fb.w as i64, fb.h as i64));
    if let Err(e) = fb.save_region_png(x, y, w, h, &out) {
        eprintln!("dump: {e}");
        return ExitCode::from(1);
    }
    eprintln!("• wrote {}", out.display());
    ExitCode::SUCCESS
}

fn cmd_record(mut args: std::iter::Skip<std::env::Args>) -> ExitCode {
    let mut common = Common { root: None, path: None, headless: false, muted: false };
    let mut out: Option<PathBuf> = None;
    let mut duration_ms: Option<u64> = None;
    let mut frames: Option<PathBuf> = None;
    let mut fps: u32 = 12;
    let mut from_ms: u64 = 0;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => match args.next() { Some(v) => out = Some(PathBuf::from(v)), None => return usage() },
            "--duration" => match args.next().and_then(|v| v.parse().ok()) { Some(v) => duration_ms = Some(v), None => return usage() },
            "--frames" => match args.next() { Some(v) => frames = Some(PathBuf::from(v)), None => return usage() },
            "--fps" => match args.next().and_then(|v| v.parse().ok()) { Some(v) => fps = v, None => return usage() },
            "--from" => match args.next().and_then(|v| v.parse().ok()) { Some(v) => from_ms = v, None => return usage() },
            "--headless" => common.headless = true,
            _ if parse_root(&mut common, &mut args, &a).unwrap_or(false) => {}
            _ => common.path = Some(a),
        }
    }
    let (Some(out), Some(duration_ms)) = (out, duration_ms) else { return usage(); };
    let Some((root, path)) = resolve_root(&common) else { return usage(); };
    // Offline render: silent (no audio device), deterministic, faster than real.
    let host = match Host::new_cart(root.clone(), common.headless, true, &path) {
        Ok(h) => h,
        Err(e) => { eprintln!("host init failed: {e}"); return ExitCode::from(1); }
    };
    let r = match runner::load_module(&path, &host) {
        Ok(r) => r,
        Err(e) => { eprintln!("{e}"); return ExitCode::from(1); }
    };
    let (module, static_names, fn_names) = (r.module, r.static_names, r.fn_names);
    match runner::run_render(module, static_names, fn_names, &host, duration_ms, &out, frames.clone(), fps, from_ms) {
        Ok(()) => {
            if let Some(dir) = &frames { eprintln!("• wrote frames to {}", dir.display()); }
            eprintln!("• wrote {}", out.display());
            ExitCode::SUCCESS
        }
        Err(e) => { eprintln!("{e}"); ExitCode::from(1) }
    }
}

fn cmd_disasm(mut args: std::iter::Skip<std::env::Args>) -> ExitCode {
    let mut common = Common { root: None, path: None, headless: true, muted: false };
    while let Some(a) = args.next() {
        match a.as_str() {
            _ if parse_root(&mut common, &mut args, &a).unwrap_or(false) => {}
            _ => common.path = Some(a),
        }
    }
    let Some((root, path)) = resolve_root(&common) else { return usage(); };
    let host = match Host::new_cart(root.clone(), common.headless, common.muted, &path) {
        Ok(h) => h,
        Err(e) => { eprintln!("host init failed: {e}"); return ExitCode::from(1); }
    };
    match posara::debug::disasm(&path, &host) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("{e}"); ExitCode::from(1) }
    }
}

fn cmd_bench(mut args: std::iter::Skip<std::env::Args>) -> ExitCode {
    let mut common = Common { root: None, path: None, headless: true, muted: false };
    let mut frames: u64 = 10000;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--frames" => match args.next().and_then(|v| v.parse().ok()) { Some(v) => frames = v, None => return usage() },
            _ if parse_root(&mut common, &mut args, &a).unwrap_or(false) => {}
            _ => common.path = Some(a),
        }
    }
    let Some((root, path)) = resolve_root(&common) else { return usage(); };
    let host = match Host::new_cart(root.clone(), true, common.muted, &path) {
        Ok(h) => h,
        Err(e) => { eprintln!("host init failed: {e}"); return ExitCode::from(1); }
    };
    let r = match runner::load_module(&path, &host) {
        Ok(r) => r,
        Err(e) => { eprintln!("{e}"); return ExitCode::from(1); }
    };
    match runner::run_until_frame(r.module, r.static_names, r.fn_names, &host, frames) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("{e}"); ExitCode::from(1) }
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else { return usage(); };
    match cmd.as_str() {
        "run"    => cmd_run(args),
        "check"  => cmd_check(args),
        "bench"  => cmd_bench(args),
        #[cfg(feature = "test")]
        "test"   => cmd_test(args),
        "disasm" => cmd_disasm(args),
        "dump"   => cmd_dump(args),
        "record" => cmd_record(args),
        "build"  => cmd_build(args),
        _        => usage(),
    }
}

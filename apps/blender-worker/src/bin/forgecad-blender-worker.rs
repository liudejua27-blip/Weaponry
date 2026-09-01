//! Fixed-layout live entry for the Weaponry Blender knife worker.
//!
//! The command line is intentionally tiny: callers select either the checked
//! in repository layout or a staged package, plus a Runtime-owned scratch
//! directory containing `input/source.glb`.  The request itself is one closed
//! JSON value on stdin.  There is no option for an executable, Python file,
//! URL, add-on, or environment.

use forgecad_blender_worker::{
    canonical_json_bytes, KnifeBlenderInstall, KnifeBlenderWorker, KnifeWorkerRequest,
    KNIFE_MAX_REQUEST_BYTES,
};
use std::env;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
enum ResourceLayout {
    Repository,
    Package,
}

#[derive(Debug)]
struct Cli {
    layout: ResourceLayout,
    root: PathBuf,
    scratch_root: PathBuf,
}

fn usage() -> &'static str {
    "Usage: forgecad-blender-worker (--package-root PATH | --repo-root PATH) --scratch-root PATH\n\nReads one closed KnifeWorkerRequest JSON value from stdin and invokes only the fixed Blender 5.2.1 worker.\n"
}

fn next_value<I>(arguments: &mut I, option: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    let value = arguments
        .next()
        .ok_or_else(|| format!("{option} requires a path"))?;
    if value.starts_with('-') {
        return Err(format!("{option} requires a path"));
    }
    Ok(value)
}

fn parse_cli<I>(arguments: I) -> Result<Option<Cli>, String>
where
    I: IntoIterator<Item = String>,
{
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let mut layout = None;
    let mut root = None;
    let mut scratch_root = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(None),
            "--package-root" => {
                if layout.is_some() {
                    return Err("exactly one fixed resource layout must be selected".to_owned());
                }
                layout = Some(ResourceLayout::Package);
                root = Some(PathBuf::from(next_value(&mut arguments, "--package-root")?));
            }
            "--repo-root" => {
                if layout.is_some() {
                    return Err("exactly one fixed resource layout must be selected".to_owned());
                }
                layout = Some(ResourceLayout::Repository);
                root = Some(PathBuf::from(next_value(&mut arguments, "--repo-root")?));
            }
            "--scratch-root" => {
                if scratch_root.is_some() {
                    return Err("--scratch-root may only be supplied once".to_owned());
                }
                scratch_root = Some(PathBuf::from(next_value(&mut arguments, "--scratch-root")?));
            }
            _ => {
                return Err(format!(
                    "unsupported option {argument}; script, executable, URL, add-on, and environment options are not accepted"
                ));
            }
        }
    }
    Ok(Some(Cli {
        layout: layout.ok_or_else(|| "select --package-root or --repo-root".to_owned())?,
        root: root.expect("layout and root are assigned together"),
        scratch_root: scratch_root.ok_or_else(|| "--scratch-root is required".to_owned())?,
    }))
}

fn read_stdin_bounded() -> Result<Vec<u8>, String> {
    let mut stdin = io::stdin().lock();
    let mut request = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = stdin
            .read(&mut buffer)
            .map_err(|error| format!("could not read closed request: {error}"))?;
        if read == 0 {
            break;
        }
        if request.len().saturating_add(read) > KNIFE_MAX_REQUEST_BYTES {
            return Err("closed request exceeds the fixed byte bound".to_owned());
        }
        request.extend_from_slice(&buffer[..read]);
    }
    if request.is_empty() {
        return Err("closed request is empty".to_owned());
    }
    Ok(request)
}

fn emit_response(response: &forgecad_blender_worker::KnifeWorkerResponse) -> Result<(), String> {
    let value = serde_json::to_value(response)
        .map_err(|error| format!("could not serialize closed response: {error}"))?;
    let bytes = canonical_json_bytes(&value)
        .map_err(|error| format!("could not canonicalize closed response: {error}"))?;
    if bytes.len() >= 64 * 1024 {
        return Err("closed response exceeds the fixed stdout bound".to_owned());
    }
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(&bytes)
        .and_then(|_| stdout.write_all(b"\n"))
        .and_then(|_| stdout.flush())
        .map_err(|error| format!("could not write closed response: {error}"))
}

fn run(cli: Cli) -> Result<(), String> {
    let install = match cli.layout {
        ResourceLayout::Repository => KnifeBlenderInstall::from_repository_root(&cli.root),
        ResourceLayout::Package => KnifeBlenderInstall::from_packaged_manifest(&cli.root),
    }
    .map_err(|error| error.to_string())?;
    let worker = KnifeBlenderWorker::new(install).map_err(|error| error.to_string())?;
    let request_bytes = read_stdin_bounded()?;
    let request: KnifeWorkerRequest = serde_json::from_slice(&request_bytes)
        .map_err(|error| format!("closed request is invalid JSON: {error}"))?;
    let run = worker
        .run_from_staged_root(&request, &cli.scratch_root)
        .map_err(|error| error.to_string())?;
    emit_response(&run.response)
}

fn main() {
    let result = match parse_cli(env::args()) {
        Ok(None) => {
            print!("{}", usage());
            Ok(())
        }
        Ok(Some(cli)) => run(cli),
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        let _ = writeln!(io::stderr().lock(), "forgecad-blender-worker: {error}");
        std::process::exit(2);
    }
}

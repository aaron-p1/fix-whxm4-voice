//! `generate`: makes changes with bash scripts that make clips with ffmpeg: the sound script generates
//! tones, and the battery script extracts short battery clips from the English originals.
//!
//! The scripts stay scripts so anyone can read and change them. The Nix package installs them next to
//! the binary and points [`SCRIPT_ENV`] and [`BATTERY_SCRIPT_ENV`] at them; setting those variables
//! runs other copies instead. On Windows they run with Git for Windows' bash (ADR 0007), see [`bash`].

use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::changes;
use crate::workspace::Workspace;

/// Environment variable naming the sound script to run.
pub const SCRIPT_ENV: &str = "FIX_WHXM4_VOICE_GENERATE_SCRIPT";
/// Environment variable naming the battery script to run.
pub const BATTERY_SCRIPT_ENV: &str = "FIX_WHXM4_VOICE_BATTERY_SCRIPT";
/// Environment variable naming the bash to run the scripts with.
pub const BASH_ENV: &str = "FIX_WHXM4_VOICE_BASH";
/// Folder in `build/` the scripts write into before their clips are moved into `changes/`.
const STAGING: &str = "generated";

/// The bash that runs the scripts: the one [`BASH_ENV`] names, or else `bash` on the PATH. On
/// Windows, the `bash` on the PATH is usually WSL's, which cannot open the Windows paths it is
/// handed, so there it is Git for Windows' bash instead, found through `git` on the PATH or in
/// Git's default install folders. Its tools (awk, mktemp, ...) come with it.
pub fn bash() -> io::Result<PathBuf> {
    if let Some(bash) = env::var_os(BASH_ENV) {
        return Ok(PathBuf::from(bash));
    }
    if cfg!(windows) {
        git_bash()
    } else {
        Ok(PathBuf::from("bash"))
    }
}

/// Git for Windows' bash: `bin\bash.exe` under Git's install folder, the launcher that puts Git's
/// `usr\bin` tools on the PATH.
fn git_bash() -> io::Result<PathBuf> {
    // Git's install folder, from `git.exe` on the PATH: it lives in `cmd\`, `bin\` or `mingw64\bin\`.
    let path = env::var_os("PATH").unwrap_or_default();
    let mut roots: Vec<PathBuf> = env::split_paths(&path)
        .filter(|dir| dir.join("git.exe").is_file())
        .flat_map(|dir| {
            let parent = dir.parent().map(Path::to_path_buf);
            let grandparent = dir.parent().and_then(Path::parent).map(Path::to_path_buf);
            [parent, grandparent]
        })
        .flatten()
        .collect();
    for (var, under) in [
        ("ProgramFiles", "Git"),
        ("ProgramFiles(x86)", "Git"),
        ("LocalAppData", "Programs\\Git"),
    ] {
        if let Some(base) = env::var_os(var) {
            roots.push(PathBuf::from(base).join(under));
        }
    }
    roots
        .iter()
        .map(|root| root.join("bin").join("bash.exe"))
        .find(|bash| bash.is_file())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Git for Windows was not found, and its bash is what runs the scripts\n\
                     Install it from https://gitforwindows.org, or set {BASH_ENV} to a bash that opens \
                     Windows paths and can run ffmpeg."
                ),
            )
        })
}

/// A script `generate` runs as `bash <path> <staging folder> <args>...`.
pub struct Script {
    pub path: PathBuf,
    pub args: Vec<OsString>,
}

impl Script {
    /// The sound script: the one [`SCRIPT_ENV`] names, or else the one in the source tree.
    pub fn sounds() -> Self {
        Script {
            path: installed(SCRIPT_ENV, "generate.sh"),
            args: Vec::new(),
        }
    }

    /// The battery script, reading the originals of `ws`: the one [`BATTERY_SCRIPT_ENV`] names, or
    /// else the one in the source tree. Its cut points fit the English originals only.
    pub fn battery(ws: &Workspace) -> Self {
        Script {
            path: installed(BATTERY_SCRIPT_ENV, "battery.sh"),
            args: vec![ws.original.clone().into_os_string()],
        }
    }
}

/// The script `var` names, or else `name` in the source tree's `sounds/`.
fn installed(var: &str, name: &str) -> PathBuf {
    env::var_os(var).map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("sounds")
            .join(name)
    })
}

/// Runs `scripts` with bash, one after another, and moves the clips they write into `changes/`.
/// Refuses, before touching `changes/`, if a script fails, or if a change for the same slot as a
/// generated clip already exists, unless `force` is set, which replaces those changes. Returns the
/// names of the generated clips.
pub fn generate(ws: &Workspace, scripts: &[Script], force: bool) -> io::Result<Vec<String>> {
    let staging = ws.build.join(STAGING);
    let result = stage(scripts, &staging).and_then(|generated| {
        replace(ws, &staging, &generated, force)?;
        Ok(generated)
    });
    let _ = fs::remove_dir_all(&staging);
    result
}

/// Runs the scripts into an empty `staging` folder and returns the names of the clips they wrote.
fn stage(scripts: &[Script], staging: &Path) -> io::Result<Vec<String>> {
    let _ = fs::remove_dir_all(staging);
    fs::create_dir_all(staging)?;
    for script in scripts {
        run(script, staging)?;
    }
    let generated: Vec<String> = changes::list(staging)?
        .into_iter()
        .map(|clip| clip.name)
        .collect();
    if let Some(name) = generated.iter().find(|name| changes::slot(name).is_none()) {
        let paths: Vec<String> = scripts
            .iter()
            .map(|script| script.path.display().to_string())
            .collect();
        return Err(io::Error::other(format!(
            "a script ({}) wrote {name}, which does not start with a slot number",
            paths.join(", ")
        )));
    }
    Ok(generated)
}

/// Runs `script` with bash, writing into `staging`.
fn run(script: &Script, staging: &Path) -> io::Result<()> {
    let path = &script.path;
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("the script {} does not exist", path.display()),
        ));
    }
    let bash = bash()?;
    let output = Command::new(&bash)
        .arg(path)
        .arg(staging)
        .args(&script.args)
        .output()
        .map_err(|e| io::Error::new(e.kind(), format!("cannot run {}: {e}", bash.display())))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!(
            "the script {} failed ({}): {}",
            path.display(),
            output.status,
            stderr.trim()
        )));
    }
    Ok(())
}

/// Moves the generated clips from `staging` into `changes/`, after removing changes for the same
/// slots if `force` is set, or refusing if it is not.
fn replace(ws: &Workspace, staging: &Path, generated: &[String], force: bool) -> io::Result<()> {
    let slots: BTreeSet<usize> = generated
        .iter()
        .filter_map(|name| changes::slot(name))
        .collect();
    let conflicts: Vec<String> = changes::list(&ws.changes)?
        .into_iter()
        .map(|change| change.name)
        .filter(|name| changes::slot(name).is_some_and(|slot| slots.contains(&slot)))
        .collect();
    if !conflicts.is_empty() && !force {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{} already holds changes for slots `generate` writes:{}\n\
                 Run with -f to replace them.",
                ws.changes.display(),
                conflicts
                    .iter()
                    .map(|name| format!("\n  {name}"))
                    .collect::<String>()
            ),
        ));
    }
    for name in &conflicts {
        fs::remove_file(ws.changes.join(name))?;
    }
    for name in generated {
        fs::rename(staging.join(name), ws.changes.join(name))?;
    }
    Ok(())
}

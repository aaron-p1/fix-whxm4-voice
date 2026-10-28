//! The completion scripts, driven through real bash, zsh and fish.
//!
//! Needs bash with programmable completion (bashInteractive), zsh and fish on the PATH, as the dev
//! shell provides them. The Nix build skips these tests. On Windows, bash is Git for Windows' bash,
//! as `generate` finds it, and the zsh and fish tests are ignored.

mod common;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use common::TempDir;
use fix_whxm4_voice::generate::bash;

const BIN: &str = env!("CARGO_BIN_EXE_fix-whxm4-voice");

const SUBCOMMANDS: &[&str] = &[
    "build",
    "completions",
    "device-info",
    "download",
    "generate",
    "help",
    "upload",
];

/// The languages `--install-lang` takes: every one with a stock pack, but Japanese and Chinese.
const INSTALL_LANGUAGES: &[&str] = &[
    "brazilian-portuguese",
    "dutch",
    "english",
    "finnish",
    "french",
    "german",
    "italian",
    "korean",
    "portuguese",
    "russian",
    "spanish",
    "swedish",
    "turkish",
];

/// The languages `--lang` takes: every one with a stock pack.
const BASE_LANGUAGES: &[&str] = &[
    "brazilian-portuguese",
    "chinese",
    "dutch",
    "english",
    "finnish",
    "french",
    "german",
    "italian",
    "japanese",
    "korean",
    "portuguese",
    "russian",
    "spanish",
    "swedish",
    "turkish",
];

/// Command lines, completed at their end, and what every shell offers there.
const CASES: &[(&str, &[&str])] = &[
    ("fix-whxm4-voice ", SUBCOMMANDS),
    (
        "fix-whxm4-voice -",
        &[
            "--dir",
            "--force",
            "--help",
            "--install-lang",
            "--lang",
            "--version",
            "-V",
            "-d",
            "-f",
            "-h",
        ],
    ),
    // An option that ends the command line.
    ("fix-whxm4-voice --version ", &[]),
    ("fix-whxm4-voice build -h -", &[]),
    // The default command's options do not lead to a subcommand.
    ("fix-whxm4-voice -d x ", &[]),
    // A given option is not offered again, in either form.
    (
        "fix-whxm4-voice build -f -",
        &["--dir", "--help", "--install-lang", "-d", "-h"],
    ),
    // Only the subcommand's own options.
    (
        "fix-whxm4-voice device-info -",
        &["--help", "--verbose", "-h", "-v"],
    ),
    // Nor one that conflicts with a given one.
    (
        "fix-whxm4-voice generate --battery --",
        &["--dir", "--force", "--help", "--lang"],
    ),
    ("fix-whxm4-voice build --install-lang ", INSTALL_LANGUAGES),
    ("fix-whxm4-voice download --lang ", BASE_LANGUAGES),
    (
        "fix-whxm4-voice build --install-lang german -",
        &["--dir", "--force", "--help", "-d", "-f", "-h"],
    ),
    ("fix-whxm4-voice upload -d s", &["sounds", "src"]),
    ("fix-whxm4-voice completions ", &["bash", "fish", "zsh"]),
    ("fix-whxm4-voice help ", SUBCOMMANDS),
    ("fix-whxm4-voice help build ", &[]),
];

fn script(shell: &str, dir: &Path) -> std::path::PathBuf {
    let out = Command::new(BIN)
        .args(["completions", shell])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // zsh finds a completion function by its file name.
    let path = dir.join(match shell {
        "zsh" => "_fix-whxm4-voice",
        other => other,
    });
    fs::write(&path, out.stdout).unwrap();
    path
}

/// A directory to complete in, holding the directories `sounds` and `src`.
fn workdir(tmp: &TempDir) -> std::path::PathBuf {
    let dir = tmp.path().join("work");
    for name in ["sounds", "src"] {
        fs::create_dir_all(dir.join(name)).unwrap();
    }
    dir
}

/// The completions in `out`, sorted, without the slash a shell adds to a directory.
fn words(out: Output, shell: &str, line: &str) -> Vec<String> {
    assert!(
        out.status.success(),
        "{shell} on {line:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut words: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|w| w.trim_end_matches('/').to_string())
        .filter(|w| !w.is_empty())
        .collect();
    words.sort();
    words.dedup();
    words
}

fn check(shell: &str, complete: impl Fn(&str) -> Vec<String>) {
    let mut failures = Vec::new();
    for (line, expected) in CASES {
        let got = complete(line);
        if got != *expected {
            failures.push(format!("{line:?}: got {got:?}, expected {expected:?}"));
        }
    }
    assert!(failures.is_empty(), "{shell}:\n{}", failures.join("\n"));
}

const BASH_DRIVER: &str = r#"
source "$1"
read -ra COMP_WORDS <<< "$2"
[[ $2 == *" " ]] && COMP_WORDS+=("")
COMP_CWORD=$(( ${#COMP_WORDS[@]} - 1 ))
COMP_LINE=$2
COMP_POINT=${#2}
_fix_whxm4_voice fix-whxm4-voice "${COMP_WORDS[COMP_CWORD]}" "${COMP_WORDS[COMP_CWORD-1]}"
printf '%s\n' "${COMPREPLY[@]}"
"#;

#[test]
fn bash_completes_by_the_rules() {
    let tmp = TempDir::new();
    let script = script("bash", tmp.path());
    let work = workdir(&tmp);
    let bash = bash().unwrap();
    let probe = Command::new(&bash)
        .args(["-c", "type compgen"])
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "{} has no programmable completion; use bashInteractive",
        bash.display()
    );
    check("bash", |line| {
        let out = Command::new(&bash)
            .args(["-c", BASH_DRIVER, "bash"])
            .arg(&script)
            .arg(line)
            .current_dir(&work)
            .output()
            .unwrap();
        words(out, "bash", line)
    });
}

/// Completes `$2` in an interactive zsh with the completion functions in `$1`, printing each
/// completion the functions add, one per line. What a bare `zsh -f` then lists can differ: with no
/// completion styles set, it leaves out some long flags that a configured zsh lists.
const ZSH_DRIVER: &str = r#"
zmodload zsh/zpty
zpty z zsh -f -i
zpty -w z "PROMPT='' RPROMPT=''; fpath=($1 \$fpath); autoload -U compinit; compinit -u -D"
zpty -w z 'compadd() { builtin compadd "$@"; local -a a; builtin compadd -O a "$@"; print -rl -- "<<"${^a} >/dev/tty }'
zpty -w z "bindkey '^I' complete-word; LISTMAX=10000"
zpty -w z "print -- ---REA''DY---"
zpty -r z out '*---READY---*'
zpty -w -n z "$2"$'\t'
zpty -w z $'\C-u'"print -- ---DO''NE---"
zpty -r z out '*---DONE---*'
zpty -d z
# A completion can follow the echo of the command line on the same line.
for word in ${=out//$'\r'/}; do
    if [[ $word == *"<<"* ]] print -r -- ${word##*<<}
done
"#;

#[test]
#[cfg_attr(windows, ignore = "needs zsh, which Windows has not")]
fn zsh_completes_by_the_rules() {
    let tmp = TempDir::new();
    let script = script("zsh", tmp.path());
    let work = workdir(&tmp);
    check("zsh", |line| {
        let out = Command::new("timeout")
            .args(["20", "zsh", "-f", "-c", ZSH_DRIVER, "zsh"])
            .arg(script.parent().unwrap())
            .arg(line)
            .current_dir(&work)
            .output()
            .unwrap();
        words(out, "zsh", line)
    });
}

#[test]
#[cfg_attr(windows, ignore = "needs fish, which Windows has not")]
fn fish_completes_by_the_rules() {
    let tmp = TempDir::new();
    let script = script("fish", tmp.path());
    let work = workdir(&tmp);
    check("fish", |line| {
        let out = Command::new("fish")
            .args(["--no-config", "-c", "source $argv[1]; complete -C $argv[2]"])
            .arg(&script)
            .arg(line)
            .current_dir(&work)
            .output()
            .unwrap();
        // Each line is a completion, then a tab and its description.
        let out = Output {
            stdout: String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|l| format!("{}\n", l.split('\t').next().unwrap_or_default()))
                .collect::<String>()
                .into_bytes(),
            ..out
        };
        words(out, "fish", line)
    });
}

#[test]
fn completions_leave_the_workspace_alone() {
    let tmp = TempDir::new();

    let out = Command::new(BIN)
        .args(["completions", "bash"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(out.status.success());
    assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
}

#[test]
fn unknown_shell_is_refused() {
    let out = Command::new(BIN)
        .args(["completions", "tcsh"])
        .output()
        .unwrap();

    assert!(!out.status.success());
}

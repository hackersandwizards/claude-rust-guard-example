//! Build script — the Cargo analog of npm's `prepare`: auto-install the git
//! hooks (lefthook) on the first local dev build, so a fresh clone never needs a
//! manual `just setup`.
//!
//! Fail-soft by design — it must never break a build:
//!   - not a git checkout (registry build, tarball, docs.rs) -> do nothing
//!   - hooks already wired -> idempotent skip
//!   - lefthook not installed -> print a hint, succeed anyway

use std::path::Path;
use std::process::Command;

fn main() {
    // Only re-run when the hook setup itself changes, not on every source edit.
    println!("cargo:rerun-if-changed=lefthook.yml");
    println!("cargo:rerun-if-changed=build.rs");

    let Ok(root) = std::env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let git = Path::new(&root).join(".git");
    if !git.exists() {
        return; // not a dev checkout — nothing to wire
    }

    // Idempotent: lefthook writes its marker into the generated hook.
    let pre_commit = git.join("hooks").join("pre-commit");
    if std::fs::read_to_string(&pre_commit)
        .map(|s| s.contains("lefthook"))
        .unwrap_or(false)
    {
        return;
    }

    // lefthook absent: hint, but don't fail the build.
    if Command::new("lefthook").arg("version").output().is_err() {
        println!(
            "cargo:warning=lefthook not found — git hooks not installed. \
             Run `brew install lefthook` then `just setup`."
        );
        return;
    }

    let _ = Command::new("lefthook")
        .arg("install")
        .current_dir(&root)
        .status();
}

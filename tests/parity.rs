//! Parity test: run the compiled binary over the zsh oracle's verdict table and
//! assert its exit code matches for every line.
//!
//! `testdata/expected.tsv` is generated from the reference zsh hook by
//! `scripts/oracle.zsh` (run via `just oracle`, which `just test` does first).
//! Each line is `<exit-code>\t<command>`, so the table is the self-contained
//! spec — the test walks it directly and never re-joins against the corpus.

use std::io::Write;
use std::process::{Command, Stdio};

/// Feed `cmd` to the binary as a hook payload; return its exit code.
fn run(cmd: &str) -> i32 {
    let payload = serde_json::json!({ "tool_input": { "command": cmd } }).to_string();
    let mut child = Command::new(env!("CARGO_BIN_EXE_claude-rg-guard"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn binary");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("write payload");
    child.wait().expect("wait").code().unwrap_or(-1)
}

#[test]
fn corpus_matches_oracle() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let expected = std::fs::read_to_string(format!("{dir}/testdata/expected.tsv"))
        .expect("read testdata/expected.tsv — run `just oracle` first");

    let mut checked = 0;
    let mut failures = Vec::new();
    for line in expected.lines() {
        let Some((code, cmd)) = line.split_once('\t') else {
            continue;
        };
        let want: i32 = code.parse().expect("exit code is an int");
        let got = run(cmd);
        if got != want {
            failures.push(format!("  {cmd:?}: oracle={want} binary={got}"));
        }
        checked += 1;
    }

    assert!(checked > 0, "expected.tsv is empty — run `just oracle`");
    assert!(
        failures.is_empty(),
        "parity failures ({}/{checked}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

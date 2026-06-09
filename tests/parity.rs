//! Parity test: run the compiled binary over the shared corpus and assert its
//! exit code matches the zsh oracle's verdict for every line.
//!
//! The oracle verdicts live in `testdata/expected.tsv`, generated from the
//! reference zsh hook by `scripts/oracle.zsh` (run via `just oracle`, which the
//! `just test` recipe does first). The corpus and the expected table are read at
//! runtime so this test never needs a stale snapshot baked into the binary.

use std::collections::HashMap;
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

fn skip(line: &str) -> bool {
    line.is_empty() || line.starts_with('#')
}

#[test]
fn corpus_matches_oracle() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let corpus = std::fs::read_to_string(format!("{dir}/testdata/corpus.txt"))
        .expect("read testdata/corpus.txt");
    let expected = std::fs::read_to_string(format!("{dir}/testdata/expected.tsv"))
        .expect("read testdata/expected.tsv — run `just oracle` first");

    // expected.tsv lines: "<exit-code>\t<command>"
    let mut oracle: HashMap<&str, i32> = HashMap::new();
    for line in expected.lines() {
        if let Some((code, cmd)) = line.split_once('\t') {
            oracle.insert(cmd, code.parse().expect("exit code is an int"));
        }
    }

    let mut failures = Vec::new();
    for cmd in corpus.lines().filter(|l| !skip(l)) {
        let want = *oracle
            .get(cmd)
            .unwrap_or_else(|| panic!("no oracle entry for {cmd:?} — regenerate expected.tsv"));
        let got = run(cmd);
        if got != want {
            failures.push(format!("  {cmd:?}: oracle={want} binary={got}"));
        }
    }
    assert!(
        failures.is_empty(),
        "parity failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

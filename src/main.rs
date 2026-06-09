//! PreToolUse guard: block `rg -r` (ripgrep's `--replace`) typed as if it were
//! grep's recursive `-r`. ripgrep recurses by default; `-r` rewrites printed
//! output, never files — so this is a quality nudge, not a safety guard.
//!
//! Reads the hook JSON on stdin, extracts `.tool_input.command`, tokenizes it,
//! and exits 2 (block, message on stderr) when a bare `-r` belongs to an `rg`
//! command. Fails open: any read/parse error or uncertainty -> exit 0 (allow).
//!
//! Non-UTF-8 stdin is a deliberate, accepted divergence from the byte-safe zsh
//! oracle: `read_to_string` rejects it -> fail open. Hook payloads are JSON
//! (UTF-8 by spec), so this never arrives from Claude Code — do not "fix" it with
//! `read_to_end` + lossy decode, which would silently alter the tokenizer input.
//!
//! The whole thing is in-process (no jaq subprocess), so the cost is a single
//! process spawn. That is NOT < 1 ms — the OS spawn floor is ~1.7 ms here — but
//! it is ~4x faster than the previous zsh+jaq hook on the block path. The common
//! path stays 0 ms: the hook `if: "Bash(*rg *-r*)"` gate prevents any spawn.

use std::io::Read;
use std::process::exit;

mod guard;
mod lexer;

use guard::Verdict;

#[derive(serde::Deserialize)]
struct Payload {
    tool_input: ToolInput,
}

#[derive(serde::Deserialize)]
struct ToolInput {
    command: String,
}

const MESSAGE: &str = "BLOCKED: `rg -r` is ripgrep's --replace, not recursive. ripgrep recurses by default — drop `-r`. If you truly want search-and-replace output, pass --replace explicitly.";

fn main() {
    // Slurp stdin. Any read error -> fail open.
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        exit(0);
    }

    // Cheap substring gate: both must appear, else allow. Mirrors the zsh
    // fallback gate and stays broader than the hook `if: "Bash(*rg *-r*)"` glob.
    if !(input.contains("rg") && input.contains("-r")) {
        exit(0);
    }

    // Extract .tool_input.command; any parse error / missing field -> fail open.
    let cmd = match serde_json::from_str::<Payload>(&input) {
        Ok(p) => p.tool_input.command,
        Err(_) => exit(0),
    };
    if cmd.is_empty() {
        exit(0);
    }

    match guard::evaluate(&lexer::tokenize(&cmd)) {
        Verdict::Block => {
            eprintln!("{MESSAGE}");
            exit(2);
        }
        Verdict::Allow => exit(0),
    }
}

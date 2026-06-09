# claude-rg-guard

A Claude Code **PreToolUse** hook that blocks a Bash command when it invokes
ripgrep with a bare `-r`, e.g. `rg -r foo`.

In grep, `-r` means recursive. In **ripgrep, `-r` is `--replace`** — and ripgrep
already recurses by default. So `rg -r foo` silently treats `foo` as a replacement
string, not a recursive search. Claude types it out of grep muscle memory. This hook
catches it, exits non-zero, and feeds back a one-line correction so Claude retries
without `-r`. It is a quality nudge, not a safety guard: `--replace` only rewrites
printed output, never files.

This is a Rust port of an earlier zsh hook. The zsh version is kept verbatim at
[`reference/rg-recursion-guard.zsh`](reference/rg-recursion-guard.zsh) as the
**parity oracle** the tests check against.

## How it works

1. Claude Code calls the hook only when the command matches the settings gate
   `if: "Bash(*rg *-r*)"` — for everything else **no process is spawned** (0 ms).
2. The binary reads the hook JSON on stdin, cheap-gates on `rg` + `-r` substrings,
   extracts `.tool_input.command` (serde_json, in-process — no `jaq` subprocess).
3. It tokenizes the command with a quote-aware lexer that mirrors zsh `${(z)}`
   (quotes retained, `$(...)`/backticks kept whole, operators split out, newline →
   `;`), then a 3-state machine (head / rg / other) attributes a bare `-r` (or an
   `-r*` cluster like `-rn`) to its owning command.
4. `rg`'s own `-r` → exit 2 + message on stderr (block). Anything else, or any
   parse/uncertainty → exit 0 (**fail open**).

### What blocks vs. allows

| Blocks (exit 2) | Allows (exit 0) |
|---|---|
| `rg -r foo`, `rg -n -r p path`, `rg -rn foo`, `rg -r=x p`, `rg foo -r` | `rg foo \| sort -r` (the `-r` is sort's) |
| `cat x \| rg -r b`, `time rg -r x`, `sudo rg -r x`, `env X=1 rg -r x` | `rg '-r' x`, `rg "-r" x` (quoted) |
| `/usr/bin/rg -r x`, `X=1 rg -r x`, `command rg -r x` | `rg -- -r` (`--` ends options), `rg --replace=x p` |
| `rg foo && rg -r bar` | `grep -r foo`, `myrg -r x`, `rg $(grep -r x) y` |

## Performance

The point of the port was speed. Measured on Apple Silicon (400 spawns each):

| path | zsh hook | rust binary |
|---|---|---|
| BLOCK (`rg -r`) | 12.26 ms | **3.15 ms** (~3.9× faster) |
| early-exit (no `rg/-r`) | 3.97 ms | **3.07 ms** |
| OS spawn floor (`/usr/bin/true`) | — | 2.65 ms |

Binary: **344 KB** (stripped, LTO).

**It is NOT < 1 ms, and cannot be.** A spawned process floors at the OS
fork+exec cost — ~2.65 ms here for a binary that does nothing. The Rust binary adds
only ~0.5 ms of actual work on top of that; the spawn dominates. The only path that
is truly sub-millisecond is the common one (any command without `rg ... -r`), where
the settings `if` gate means **nothing spawns at all**.

## Build & wire-up

```sh
just build           # cargo build --release
just install         # prints the binary path to put in settings.json
```

In `~/.claude/settings.json`, the PreToolUse hook points at the release binary:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command",
            "if": "Bash(*rg *-r*)",
            "command": "/Users/bstemmildt/opt/claude-rg-guard/target/release/claude-rg-guard" }
        ]
      }
    ]
  }
}
```

The binary must be rebuilt after edits (`just build`) and per machine. If it is
missing, the hook simply fails open — no guard, no breakage.

## Tests

```sh
just test            # unit tests + corpus parity vs the zsh oracle
just fuzz 2000       # differential fuzz: 2000 random commands, rust verdict == zsh verdict
just bench           # the table above
just all             # test + fuzz + bench
```

- `testdata/corpus.txt` — shared inputs; `scripts/oracle.zsh` runs them through the
  reference zsh hook to produce `testdata/expected.tsv` (ground truth).
- `tests/parity.rs` — runs the binary over the corpus, asserts exit code == oracle.
- `scripts/fuzz.zsh` — grammar-generates commands and diffs the binary against the
  zsh hook live. Last run: **2000 cases, 0 divergences.**

## Layout

```
src/main.rs    stdin → gate → JSON extract → lex → state machine → exit 0/2
src/lexer.rs   (z)-parity tokenizer
src/guard.rs   3-state machine, ported line-for-line from the zsh oracle
reference/     the zsh hook, frozen as the parity oracle
testdata/      corpus.txt + generated expected.tsv
scripts/       oracle.zsh · fuzz.zsh · bench.zsh
```

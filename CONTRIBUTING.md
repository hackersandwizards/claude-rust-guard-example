# Contributing

How to build, check and test this repository. What it is and how to wire it into Claude Code: [README.md](README.md).

## Development

You need Rust through rustup (`rust-toolchain.toml` selects stable with rustfmt and
clippy), [just](https://github.com/casey/just), and for the tests zsh and
[jaq](https://github.com/01mf02/jaq).

Tooling:

| concern | tool | command |
|---|---|---|
| format | rustfmt (`rustfmt.toml`) | `just fmt` · `just fmt-check` |
| lint | clippy (`[lints]` in `Cargo.toml`) | `just lint` |
| test | cargo test | `just test` |
| check | cargo check | `just check` |
| git hooks | lefthook (`lefthook.yml`) | `just setup` |

```sh
brew install lefthook   # one-time, if not already present
cargo build             # build.rs auto-installs the hooks (npm-`prepare` style)
just verify             # fmt-check + lint + test + fuzz, by hand
```

The hooks install themselves: `build.rs` runs `lefthook install` on the first
local `cargo build`/`cargo test` (fail-soft: without lefthook it prints a Cargo warning and skips; outside a git
checkout it skips quietly). `just setup` stays as a manual fallback.

`lefthook.yml` runs `cargo fmt` + `cargo clippy -D warnings` (and `zsh -n` on the
shell scripts) on **pre-commit**, and `cargo test` on **pre-push**. The toolchain
channel (stable) and its `rustfmt`/`clippy` components are set in `rust-toolchain.toml`. There
is intentionally no CI — the hooks are the gate.

## Tests

```sh
just test            # unit tests + corpus parity vs the zsh oracle
just fuzz 2000       # differential fuzz: 2000 random commands, rust verdict == zsh verdict
just bench           # the table in docs/performance.md
just all             # verify (fmt-check + lint + test + fuzz) + bench
```

- `testdata/corpus.txt` — shared inputs; `scripts/oracle.zsh` runs them through the
  reference zsh hook to produce `testdata/expected.tsv` (ground truth).
- `tests/parity.rs` — runs the binary over the corpus, asserts exit code == oracle.
- `scripts/fuzz.zsh` — grammar-generates commands (heads, reserved-word/wrapper/
  assignment prefixes, balanced subshells, quotes, `$()`/backticks, `\r`, escapes,
  `#`) and diffs the binary against the zsh hook live. Last run: **20000 cases, 0
  divergences.**

## Layout

```
src/main.rs    stdin → gate → JSON extract → lex → state machine → exit 0/2
src/lexer.rs   (z)-parity tokenizer
src/guard.rs   3-state machine, ported line-for-line from the zsh oracle
reference/     the zsh hook, frozen as the parity oracle
testdata/      corpus.txt + generated expected.tsv
scripts/       oracle.zsh · fuzz.zsh · bench.zsh
```


# claude-rust-guard-example

An example of a Claude Code **PreToolUse** hook written in Rust: a port of a zsh hook, held to the original by parity tests and a differential fuzzer. It is for people who write their own hooks and want to study the technique. The hook works, but nothing depends on it and there are no releases.

The hook blocks a Bash command when it invokes ripgrep with a bare `-r`, e.g. `rg -r foo`. In grep, `-r` means recursive. In **ripgrep, `-r` is `--replace`**, and ripgrep already recurses by default. So `rg -r foo` silently treats `foo` as a replacement string, not a recursive search. Claude types it out of grep muscle memory. The hook catches it, exits non-zero, and feeds back a one-line correction so Claude retries without `-r`. It is a quality nudge, not a safety guard: `--replace` only rewrites printed output, never files.

| You want | Go to |
| --- | --- |
| See how the hook decides | [How it works](#how-it-works) |
| See what it blocks and allows | [docs/behaviour.md](docs/behaviour.md) |
| Try it in Claude Code | [Build and wire up](#build-and-wire-up) |
| Read the code | [Where to look](#where-to-look) |
| Learn the hook contract | [Claude Code's hooks reference](https://code.claude.com/docs/en/hooks) |
| Know what the port gained | [docs/performance.md](docs/performance.md) |
| Run the tests, the git hooks, change it | [CONTRIBUTING.md](CONTRIBUTING.md) |

## How it works

1. Claude Code calls the hook only when the command matches the settings gate `if: "Bash(*rg *-r*)"`. For everything else **no process is spawned** (0 ms).
2. The binary reads the hook JSON on stdin, cheap-gates on `rg` + `-r` substrings, and extracts `.tool_input.command` in-process with serde_json.
3. A quote-aware lexer that mirrors zsh `${(z)}` tokenizes the command. A 3-state machine (head / rg / other) then attributes a bare `-r`, or an `-r*` cluster like `-rn`, to its owning command.
4. `rg`'s own `-r` → exit 2 and a message on stderr (block). Anything else, or stdin that is not hook JSON with a command → exit 0 (**fail open**).

A blocked call takes 3.15 ms against 12.26 ms for the zsh hook, most of it the cost of spawning a process ([docs/performance.md](docs/performance.md)).

## Build and wire up

You need Rust through rustup (`rust-toolchain.toml` selects stable) and [just](https://github.com/casey/just).

```sh
just build
just install
```

`just build` builds the release binary. `just install` installs nothing: it prints the binary's full path. Add the hook to `~/.claude/settings.json`, where `<binary>` is that path, `<clone>/target/release/claude-rg-guard` with `<clone>` this repository's directory:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "if": "Bash(*rg *-r*)",
        "command": "<binary>"
      }]
    }]
  }
}
```

The binary must be rebuilt after edits (`just build`) and per machine. If it is missing, the hook simply fails open: no guard, no breakage.

## Where to look

- [`src/main.rs`](src/main.rs): stdin → gate → JSON extract → lex → state machine → exit 0/2.
- [`src/lexer.rs`](src/lexer.rs): the tokenizer and the zsh `(z)` rules it reproduces.
- [`src/guard.rs`](src/guard.rs): the 3-state machine, ported line-for-line from the zsh hook.
- [`reference/rg-recursion-guard.zsh`](reference/rg-recursion-guard.zsh): the original zsh hook, kept verbatim as the **parity oracle** the tests check against.
- [`tests/parity.rs`](tests/parity.rs) and [`scripts/fuzz.zsh`](scripts/fuzz.zsh): the parity test and the differential fuzzer.

Claude Code documents hooks, the `if` gate and exit code 2 in its [hooks reference](https://code.claude.com/docs/en/hooks).

## License

[PolyForm Strict 1.0.0](LICENSE), copyright hackers&wizards GmbH: you may use it for noncommercial purposes, but not distribute it or make changes or new works based on it. The name hackers&wizards and its logos are not licensed.

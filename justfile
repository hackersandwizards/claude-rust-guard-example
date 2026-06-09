# claude-rg-guard — task runner

_default:
    @just --list

# build the release binary
build:
    cargo build --release

# regenerate the oracle verdict table from the reference zsh hook
oracle:
    zsh scripts/oracle.zsh

# unit tests + corpus parity against the zsh oracle
test: build oracle
    cargo test

# differential fuzz: random commands, rust vs zsh verdicts must agree
fuzz n="500": build
    zsh scripts/fuzz.zsh {{n}}

# benchmark spawn floor vs zsh hook vs rust binary
bench n="300": build
    zsh scripts/bench.zsh {{n}}

# everything: build, oracle, test, fuzz, bench
all: test fuzz bench

# print the path to wire into ~/.claude/settings.json
install: build
    @echo 'Point the PreToolUse hook "command" in ~/.claude/settings.json at:'
    @echo '{{justfile_directory()}}/target/release/claude-rg-guard'

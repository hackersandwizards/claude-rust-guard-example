# claude-rg-guard — task runner (the Rust analog of the reference repo's npm scripts)

_default:
    @just --list

# install git hooks (needs lefthook: `brew install lefthook`)
setup:
    lefthook install

# build the release binary
build:
    cargo build --release

# format the code
fmt:
    cargo fmt

# check formatting without writing (CI-style)
fmt-check:
    cargo fmt --check

# lint with clippy (warnings are errors)
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# auto-fix lints, then format
fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features
    cargo fmt

# type/borrow check without producing artifacts
check:
    cargo check --all-targets

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

# the pre-commit/pre-push gate, run by hand: format, lint, test, fuzz
verify: fmt-check lint test fuzz

# verify + benchmark
all: verify bench

# print the path to wire into ~/.claude/settings.json
install: build
    @echo 'Point the PreToolUse hook "command" in ~/.claude/settings.json at:'
    @echo '{{justfile_directory()}}/target/release/claude-rg-guard'

# Shared helpers for the oracle / fuzz / bench scripts.
# Source from a sibling script:  source ${0:A:h}/_lib.zsh
#
# Sets: repo, hook, bin.  Provides: payload, need, require_built.
# `${0:A:h:h}` resolves to the repo root whether $0 is this lib or the caller —
# both live in scripts/, so two `:h` steps land on the repo either way.

repo=${0:A:h:h}
hook=$repo/reference/rg-recursion-guard.zsh
bin=$repo/target/release/claude-rg-guard

# Wrap a raw command as a PreToolUse hook payload (the JSON shape the hook reads).
# Single source of truth for the {tool_input:{command}} contract.
payload() { jaq -n --arg c "$1" '{tool_input:{command:$c}}' }

# Fail fast unless a required command is on PATH.
need() { (( $+commands[$1] )) || { print -u2 "${ZSH_ARGZERO:t}: $1 not found"; exit 1 } }

# Fail fast unless the release binary is built.
require_built() {
  [[ -x $bin ]] || { print -u2 "${ZSH_ARGZERO:t}: build first (cargo build --release)"; exit 1 }
}

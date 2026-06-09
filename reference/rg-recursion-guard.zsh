#!/bin/zsh -f
# PreToolUse guard: block `rg -r` (ripgrep --replace) used as if recursive.
# ripgrep recurses by default; `-r` means --replace (it changes printed output,
# never modifies files) — so this is a quality nudge, not a safety guard.
# Fails open: any tooling error or uncertainty -> exit 0 (allow).
#
# NOTE: this script is the PARITY ORACLE for the Rust port. It is no longer the
# live hook — the compiled binary is. Keep it byte-for-byte faithful to the
# behaviour the Rust code must reproduce; the test suite diffs the two.
#
# Shebang uses system /bin/zsh directly (macOS-only) + -f (skip rc files) for
# fastest cold start; PATH is inherited, so jaq/jq still resolve.
#
# Gating: the hook's `if: "Bash(*rg *-r*)"` (settings.json) is the primary,
# zero-spawn filter. The substring gate below is the pre-v2.1.85 fallback and
# MUST stay broader than that glob (it is: `*rg* && *-r*` ⊇ `*rg *-r*`).
#
# Known fail-open misses (all non-destructive): rg inside $(...)/backticks;
# wrappers with their own args (timeout N, nice -n N); `-r` not adjacent to the
# dash (rg -nr, where the `if` glob never fires); a leading redirection before
# the command; an aliased \rg.
emulate -L zsh

IFS= read -rd '' input   # slurp all of stdin via the read builtin (no fork)

# Fallback gate (cheap): bail unless both substrings appear anywhere.
[[ $input == *rg* && $input == *-r* ]] || exit 0

# Extract the Bash command. Prefer jaq, fall back to jq, else fail open.
local cmd=""
if (( $+commands[jaq] )); then
  cmd=$(jaq -r '.tool_input.command // empty' <<< "$input" 2>/dev/null)
elif (( $+commands[jq] )); then
  cmd=$(jq  -r '.tool_input.command // empty' <<< "$input" 2>/dev/null)
else
  exit 0
fi
[[ -n $cmd ]] || exit 0

# Quote-aware shell tokenization. (z) keeps quotes (and $(...)/backticks) inside
# tokens and emits operators (| || && ; & ...) as their own tokens; a newline
# becomes a `;` token.
local -a toks
toks=( ${(z)cmd} )

# Walk tokens, attributing a bare `-r` to its owning command. Three states:
#   head / rg / other
local state=head t pre
for t in $toks; do
  if [[ $t == ('|'|'||'|'&&'|';'|';;'|';&'|';|'|'&'|'|&'|'('|')'|'{'|'}') ]]; then
    state=head; continue
  fi
  if [[ $state == head ]]; then
    [[ $t == (do|then|else|elif) ]] && continue
    if [[ $t == *=* ]]; then
      pre=${t%%=*}
      [[ $pre == [A-Za-z_]* && $pre != *[^A-Za-z0-9_]* ]] && continue
    fi
    [[ $t == (sudo|doas|command|builtin|env|time|nice|nohup|xargs|stdbuf|nocorrect|noglob) ]] && continue
    [[ $t == rg || $t == */rg ]] && state=rg || state=other
    continue
  fi
  [[ $state == rg ]] || continue
  [[ $t == '--' ]] && { state=other; continue; }
  if [[ $t == -r* ]]; then
    print -u2 -- "BLOCKED: \`rg -r\` is ripgrep's --replace, not recursive. ripgrep recurses by default — drop \`-r\`. If you truly want search-and-replace output, pass --replace explicitly."
    exit 2
  fi
done

exit 0

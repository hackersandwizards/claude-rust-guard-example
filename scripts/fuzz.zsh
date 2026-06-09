#!/bin/zsh -f
# Differential fuzzer: generate random shell-ish commands and assert the Rust
# binary and the reference zsh hook return the SAME verdict for each. Any
# divergence is a parity bug in the lexer or state machine.
#
# Deterministic: RANDOM is seeded, so a reported divergence reproduces.
emulate -L zsh
source ${0:A:h}/_lib.zsh
require_built
need jaq

local -a heads prefixes args ops
heads=(rg grep /usr/bin/rg myrg sudo env time command cat sort)
# 0-2 of these precede a head: command-position-preserving reserved words, the
# guard's wrappers, and assignments. Exercises the lexer's command-position
# tracking (which decides whether a following '(' splits) and the guard's
# head-skipping. Kept syntactically plausible (no stray/unbalanced parens —
# zsh tokenizes malformed input idiosyncratically and it never occurs for real).
# (for/case/repeat/function are omitted: zsh gives them special paren syntax —
# `for x (list)`, `case pat)` — which the lexer deliberately does not model.)
prefixes=('' '' 'sudo' 'doas' 'command' 'builtin' 'env' 'nice' 'nohup' 'xargs' 'time' 'nocorrect' 'if' 'while' 'until' 'do' 'then' 'else' 'elif' 'done' 'fi' 'esac' '!' '{' '}' 'X=1' 'Y=2')
# args include BALANCED paren groups (so a '(' appears in argument position), a
# backslash-escape, and a '#' (zsh (z) does NOT treat it as a comment). Complex
# redirections (>& 2>&1 &>) are intentionally NOT fuzzed — they are an unmodeled,
# fail-open class (see src/lexer.rs); the common `cmd ... > file` is pinned in the
# corpus instead.
args=(-r -rn '-r=x' "'-r'" '"-r"' -- --replace '--replace=x' foo 'bar baz' $'fo\ro' $'-r\rx' '$(grep -r x)' '`grep -r x`' '(rg -r x)' '(bar)' '(rg foo)' '\-r' '#c')
ops=('|' '&&' '||' ';' '&' ' ')   # ' ' = no operator, just continue the command

local N=${1:-500}
RANDOM=12345
local diffs=0 k seg nseg a narg p npre pre cmd json zexit rexit wrap

for (( k = 0; k < N; k++ )); do
  cmd=""
  nseg=$(( RANDOM % 3 + 1 ))
  for (( seg = 0; seg < nseg; seg++ )); do
    (( seg > 0 )) && cmd+=" ${ops[RANDOM % ${#ops} + 1]} "
    # 1 in 4 segments is a balanced subshell: ( ... ) — a command-position '('.
    wrap=$(( RANDOM % 4 == 0 ))
    (( wrap )) && cmd+="( "
    npre=$(( RANDOM % 3 ))
    for (( p = 0; p < npre; p++ )); do
      pre=${prefixes[RANDOM % ${#prefixes} + 1]}
      [[ -n $pre ]] && cmd+="$pre "
    done
    cmd+="${heads[RANDOM % ${#heads} + 1]}"
    narg=$(( RANDOM % 4 ))
    for (( a = 0; a < narg; a++ )); do
      cmd+=" ${args[RANDOM % ${#args} + 1]}"
    done
    (( wrap )) && cmd+=" )"
  done

  json=$(payload "$cmd")
  print -r -- "$json" | $hook >/dev/null 2>&1; zexit=$?
  print -r -- "$json" | $bin  >/dev/null 2>&1; rexit=$?
  if [[ $zexit != $rexit ]]; then
    print -r -- "DIFF zsh=$zexit rust=$rexit :: $cmd"
    (( diffs++ ))
  fi
done

print "fuzz: $N cases, $diffs divergence(s)"
(( diffs == 0 ))

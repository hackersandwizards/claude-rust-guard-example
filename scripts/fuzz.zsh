#!/bin/zsh -f
# Differential fuzzer: generate random shell-ish commands and assert the Rust
# binary and the reference zsh hook return the SAME verdict for each. Any
# divergence is a parity bug in the lexer or state machine.
#
# Deterministic: RANDOM is seeded, so a reported divergence reproduces.
emulate -L zsh

repo=${0:A:h:h}
hook=$repo/reference/rg-recursion-guard.zsh
bin=$repo/target/release/claude-rg-guard
[[ -x $bin ]] || { print -u2 "fuzz: build first (cargo build --release)"; exit 1; }
(( $+commands[jaq] )) || { print -u2 "fuzz: jaq not found"; exit 1; }

local -a heads args ops
heads=(rg grep /usr/bin/rg myrg sudo env time command cat sort)
args=(-r -rn '-r=x' "'-r'" '"-r"' -- --replace '--replace=x' foo 'bar baz' '$(grep -r x)' '`grep -r x`')
ops=('|' '&&' '||' ';' '&' ' ')   # ' ' = no operator, just continue the command

local N=${1:-500}
RANDOM=12345
local diffs=0 k seg nseg a narg cmd json zexit rexit

for (( k = 0; k < N; k++ )); do
  cmd=""
  nseg=$(( RANDOM % 3 + 1 ))
  for (( seg = 0; seg < nseg; seg++ )); do
    (( seg > 0 )) && cmd+=" ${ops[RANDOM % ${#ops} + 1]} "
    cmd+="${heads[RANDOM % ${#heads} + 1]}"
    narg=$(( RANDOM % 4 ))
    for (( a = 0; a < narg; a++ )); do
      cmd+=" ${args[RANDOM % ${#args} + 1]}"
    done
  done

  json=$(jaq -n --arg c "$cmd" '{tool_input:{command:$c}}')
  print -r -- "$json" | $hook >/dev/null 2>&1; zexit=$?
  print -r -- "$json" | $bin  >/dev/null 2>&1; rexit=$?
  if [[ $zexit != $rexit ]]; then
    print -r -- "DIFF zsh=$zexit rust=$rexit :: $cmd"
    (( diffs++ ))
  fi
done

print "fuzz: $N cases, $diffs divergence(s)"
(( diffs == 0 ))

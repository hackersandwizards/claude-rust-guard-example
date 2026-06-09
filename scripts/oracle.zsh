#!/bin/zsh -f
# Generate the ground-truth verdict table from the reference zsh hook.
# For each corpus line, wrap it as a hook payload and record the hook's exit code.
# Output: testdata/expected.tsv  ("<exit-code>\t<command>")
emulate -L zsh
# No err_exit: the hook exits 2 on a BLOCK, which is exactly what we record.

repo=${0:A:h:h}
hook=$repo/reference/rg-recursion-guard.zsh
corpus=$repo/testdata/corpus.txt
out=$repo/testdata/expected.tsv

(( $+commands[jaq] )) || { print -u2 "oracle: jaq not found"; exit 1; }
[[ -r $corpus ]] || { print -u2 "oracle: missing $corpus"; exit 1; }

: > $out
# Declare locals ONCE: re-running `local` inside a zsh loop prints the variable.
local n=0 line json rc
while IFS= read -r line; do
  [[ -z $line || $line == \#* ]] && continue
  json=$(jaq -n --arg c "$line" '{tool_input:{command:$c}}')
  # The hook reads its payload from the pipe; capture its exit code (fail open=0).
  print -r -- "$json" | $hook >/dev/null 2>&1
  rc=$?
  printf '%s\t%s\n' "$rc" "$line" >> $out
  (( n++ ))
done < $corpus

print "oracle: wrote $n verdicts -> $out"

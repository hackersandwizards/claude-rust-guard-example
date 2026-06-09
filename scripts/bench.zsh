#!/bin/zsh -f
# Benchmark: OS spawn floor vs. the reference zsh hook vs. the Rust binary, on
# both the block path (rg -r) and the early-exit path (no rg/-r).
emulate -L zsh
zmodload zsh/datetime
source ${0:A:h}/_lib.zsh
require_built
need jaq

local N=${1:-300}
# Assemble the flag from pieces so this script's own command line never contains
# a literal `rg -r` (would trip the live hook when launched via the Bash tool).
local g=rg f=-$(print r)
local block_json plain_json
block_json=$(payload "$g $f foo")
plain_json=$(payload "ls -la /tmp")

run_n() {  # run_n <stdin-data> <cmd> [args...]
  local data=$1; shift
  local t0=$EPOCHREALTIME i
  for (( i = 0; i < N; i++ )); do print -r -- "$data" | "$@" >/dev/null 2>&1; done
  local t1=$EPOCHREALTIME
  printf '%8.3f ms/call\n' "$(( (t1 - t0) * 1000.0 / N ))"
}

print "N=$N per measurement\n"
printf '%-32s ' "/usr/bin/true (spawn floor)"; run_n "x"          /usr/bin/true
printf '%-32s ' "zsh hook: BLOCK (rg -r)"   ; run_n "$block_json" $hook
printf '%-32s ' "zsh hook: early-exit"      ; run_n "$plain_json" $hook
printf '%-32s ' "rust bin: BLOCK (rg -r)"   ; run_n "$block_json" $bin
printf '%-32s ' "rust bin: early-exit"      ; run_n "$plain_json" $bin

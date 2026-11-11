# Behaviour

Which commands the hook blocks and which it lets through. `testdata/corpus.txt` holds the full set the tests check.

## What blocks vs. allows

| Blocks (exit 2) | Allows (exit 0) |
|---|---|
| `rg -r foo`, `rg -n -r p path`, `rg -rn foo`, `rg -r=x p`, `rg foo -r` | `rg foo \| sort -r` (the `-r` is sort's) |
| `cat x \| rg -r b`, `time rg -r x`, `sudo rg -r x`, `env X=1 rg -r x` | `rg '-r' x`, `rg "-r" x` (quoted) |
| `/usr/bin/rg -r x`, `X=1 rg -r x`, `command rg -r x` | `rg -- -r` (`--` ends options), `rg --replace=x p` |
| `rg foo && rg -r bar` | `grep -r foo`, `myrg -r x`, `rg $(grep -r x) y` |
| `(rg -r x)`, `time (rg -r x)` (command-position subshell) | `foo (rg -r x)`, `sudo (rg -r x)` (`(` is an argument) |

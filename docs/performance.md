# Performance

The point of the port was speed. Measured on Apple Silicon (400 spawns each):

| path | zsh hook | rust binary |
|---|---|---|
| BLOCK (`rg -r`) | 12.26 ms | **3.15 ms** (~3.9× faster) |
| early-exit (no `rg/-r`) | 3.97 ms | **3.07 ms** |
| OS spawn floor (`/usr/bin/true`) | — | 2.65 ms |

Binary: **344 KB** (stripped, LTO).

**It is NOT < 1 ms, and cannot be.** A spawned process floors at the OS
fork+exec cost — ~2.65 ms here for a binary that does nothing. The Rust binary adds
only ~0.5 ms of actual work on top of that; the spawn dominates. The only path that
is truly sub-millisecond is the common one (any command without `rg ... -r`), where
the settings `if` gate means **nothing spawns at all**.

//! State machine over tokens: attribute a bare `-r` to its owning command and
//! decide block vs allow. Ported line-for-line from
//! `reference/rg-recursion-guard.zsh` (the parity oracle).

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Verdict {
    Allow,
    Block,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum State {
    /// next token is a command head
    Head,
    /// inside an `rg` invocation — watch for a `-r` flag
    Rg,
    /// inside some other command — ignore until the next operator
    Other,
}

/// Operators / separators that start a new command. This set must agree with
/// the lexer's split set and the zsh oracle's op list. Note `{` / `}` are never
/// split by the lexer — they reach here only as whitespace-delimited words
/// (e.g. `{ ... }`), exactly as in zsh, so they are matched but never produced
/// from inside a larger token.
fn is_operator(t: &str) -> bool {
    matches!(
        t,
        "|" | "||" | "&&" | ";" | ";;" | ";&" | ";|" | "&" | "|&" | "(" | ")" | "{" | "}"
    )
}

/// Reserved words that precede a command (loop/conditional bodies): transparent.
fn is_reserved(t: &str) -> bool {
    matches!(t, "do" | "then" | "else" | "elif")
}

/// Argless command wrappers whose real head follows. Wrappers that take their
/// own args (timeout N, nice -n N) are deliberately absent — the arg would be
/// read as the head (an accepted fail-open miss, same as the zsh oracle).
fn is_wrapper(t: &str) -> bool {
    matches!(
        t,
        "sudo"
            | "doas"
            | "command"
            | "builtin"
            | "env"
            | "time"
            | "nice"
            | "nohup"
            | "xargs"
            | "stdbuf"
            | "nocorrect"
            | "noglob"
    )
}

/// `NAME=value` assignment prefix: NAME starts with a letter/underscore and is
/// otherwise all word chars. Matches the zsh test
/// `[[ $pre == [A-Za-z_]* && $pre != *[^A-Za-z0-9_]* ]]`.
fn is_assignment(t: &str) -> bool {
    match t.split_once('=') {
        Some((pre, _)) => {
            !pre.is_empty()
                && pre
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && pre.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        None => false,
    }
}

/// `rg` or any path ending in `/rg` (mirrors `[[ $t == rg || $t == */rg ]]`).
fn is_rg_head(t: &str) -> bool {
    t == "rg" || t.ends_with("/rg")
}

/// Decide whether the tokenized command misuses `rg -r`.
///
/// Callers gate upstream: `main` only reaches here for inputs containing both
/// `rg` and `-r` (and Claude Code's `if: "Bash(*rg *-r*)"` gates before any
/// spawn). This function still re-derives the verdict from scratch — the gate is
/// only a fast path, never part of the decision.
pub fn evaluate(tokens: &[String]) -> Verdict {
    let mut state = State::Head;
    for tok in tokens {
        let t = tok.as_str();
        if is_operator(t) {
            state = State::Head;
            continue;
        }
        match state {
            State::Head => {
                if is_reserved(t) || is_assignment(t) || is_wrapper(t) {
                    continue;
                }
                state = if is_rg_head(t) {
                    State::Rg
                } else {
                    State::Other
                };
            }
            State::Rg => {
                if t == "--" {
                    state = State::Other; // end of options; rest are patterns/paths
                } else if t.starts_with("-r") {
                    // -r and clusters that all mean --replace: -rn, -r=x, -rfoo
                    return Verdict::Block;
                }
            }
            State::Other => {}
        }
    }
    Verdict::Allow
}

#[cfg(test)]
mod tests {
    use super::{evaluate, is_assignment, Verdict};
    use crate::lexer::tokenize;

    fn v(s: &str) -> Verdict {
        evaluate(&tokenize(s))
    }

    // The exhaustive verdict matrix lives in testdata/corpus.txt and is checked
    // end-to-end against the zsh oracle by tests/parity.rs. The cases here pin
    // each evaluate() *branch* in isolation (zsh-free), one representative apiece
    // — not a second copy of the corpus.
    #[test]
    fn blocks_bare_r_owned_by_rg() {
        for s in [
            "rg -r foo",                    // Rg state, bare -r
            "rg -n -r pat",                 // a non-matching flag, then -r
            "rg foo -r",                    // -r after a positional
            "rg -rn foo",                   // cluster -rn
            "rg -r=x p",                    // cluster -r=x
            "rg -rfoo",                     // cluster -rfoo
            "sudo rg -r x",                 // wrapper skipped -> rg head
            "X=1 rg -r x",                  // assignment skipped -> rg head
            "X=1 Y=2 rg -r x",              // consecutive assignments skipped
            "/usr/bin/rg -r x",             // path head (*/rg)
            "cat x | rg -r b",              // operator resets to Head, then rg
            "rg a && rg -r b",              // re-entry into Rg after an operator
            "for f in a; do rg -r x; done", // reserved word `do` is transparent
        ] {
            assert_eq!(v(s), Verdict::Block, "expected BLOCK for {s:?}");
        }
    }

    #[test]
    fn allows_when_r_is_not_rgs_recursive_flag() {
        for s in [
            "rg -- -r",          // -- ends options
            "rg --replace=x p",  // --replace starts with --, not -r
            "rg -R foo",         // capital R is not -r
            "rg '-r' x",         // quoted: lexer keeps the quotes, not bare
            "rg \"-r\" x",       // quoted (double)
            "rg $(grep -r x) y", // $(...) stays one opaque token
            "grep -r foo",       // non-rg head -> Other
            "myrg -r x",         // head is neither rg nor */rg
            "1x=1 rg -r x",      // invalid assignment -> non-rg head -> Other
            "rg foo | sort -r",  // -r owned by sort (Other)
            "rg foo",            // no -r at all
            "ls -la",            // no rg at all
        ] {
            assert_eq!(v(s), Verdict::Allow, "expected ALLOW for {s:?}");
        }
    }

    // is_assignment carries the only non-trivial helper logic, and its rejection
    // branches are awkward to reach through a whole command — pin them directly.
    #[test]
    fn assignment_prefix_recognition() {
        assert!(is_assignment("X=1")); // simple NAME=value
        assert!(is_assignment("FOO_BAR=baz")); // underscores + digits in name
        assert!(is_assignment("_x=1")); // leading underscore is allowed
        assert!(is_assignment("A=")); // empty value is still an assignment
        assert!(!is_assignment("1x=1")); // name must not start with a digit
        assert!(!is_assignment("a-b=1")); // `-` is not a word char
        assert!(!is_assignment("=x")); // empty name
        assert!(!is_assignment("rg")); // no `=` at all
    }
}

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
    use super::{evaluate, Verdict};
    use crate::lexer::tokenize;

    fn v(s: &str) -> Verdict {
        evaluate(&tokenize(s))
    }

    #[test]
    fn blocks_rg_replace() {
        for s in [
            "rg -r foo",
            "rg -n -r pat path",
            "cat x | rg -r b",
            "time rg -r x",
            "sudo rg -r x",
            "env X=1 rg -r x",
            "/usr/bin/rg -r x",
            "rg -rn foo",
            "rg -r=x p",
            "rg -rfoo",
            "rg foo -r",
            "X=1 rg -r x",
            "command rg -r x",
            "rg --foo -r x",
            "rg foo && rg -r bar",
        ] {
            assert_eq!(v(s), Verdict::Block, "expected BLOCK for {s:?}");
        }
    }

    #[test]
    fn allows_everything_else() {
        for s in [
            "rg foo | sort -r",
            "rg foo | xargs rm -r",
            "rg 'a -r b'",
            "rg '-r' x",
            "rg \"-r\" x",
            "grep -r foo",
            "rg --replace=x p",
            "rg -- -r",
            "rg foo",
            "rg foo; rm -r d",
            "rg foo && rm -r d",
            "rg $(grep -r x) y",
            "rg `grep -r x` y",
            "myrg -r x",
            "rgx -r foo",
            "ls -la",
            "rg -R foo",
            "echo rg -r foo",
        ] {
            assert_eq!(v(s), Verdict::Allow, "expected ALLOW for {s:?}");
        }
    }
}

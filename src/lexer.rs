//! Quote-aware tokenizer matching zsh `${(z)}` closely enough to reproduce the
//! guard's block/allow verdicts. Behaviour captured empirically from `(z)` and
//! enforced by a differential fuzzer (`scripts/fuzz.zsh`).
//!
//! Captured rules:
//! - single/double quotes are RETAINED in the token, so a quoted `-r` is not bare
//! - `$(...)`, `${...}`, and backtick spans stay inside one word token
//! - command operators split into their own tokens, even without surrounding
//!   whitespace: `|` `||` `|&` `&` `&&` `;` `;;` `;&` `;|`
//! - a newline becomes a `;` token
//! - a carriage return `\r` is an ordinary word character: zsh `(z)` retains it
//!   inside the token and never treats it as a separator. Only ` `, `\t`, and
//!   `\n` break words.
//! - `(` / `)` are POSITION-DEPENDENT. A `(` opens a subshell (its own token)
//!   only in command position; used as an argument it is a glob group glued into
//!   the word — so `foo (rg -r x)` keeps `(rg -r x)` as ONE token (and the `rg`
//!   inside is not a command head), exactly as zsh does. Command position is the
//!   start of input, just after an operator, or after an assignment / a reserved
//!   word that keeps it (see `preserves_cmd_pos`). A `)` always ends the word and
//!   splits into its own token.
//!
//! `{` / `}` are NOT split here — in zsh they are reserved words only when
//! whitespace-delimited, so they already surface as standalone tokens and the
//! state machine treats them as operators.
//!
//! Out of scope (rare, fail-closed): arithmetic `(( ))`, the internals of
//! `[[ ]]`, and `case` patterns are not modeled, so a literal `rg -r` inside one
//! of those can over-block. None of these occur in real ripgrep usage.
//!
//! Redirections are also not modeled: `&` in `>&` / `2>&1` / `&>` is treated as a
//! background operator, and `>file` as a plain word. This only ever fails OPEN
//! (a `-r` separated from `rg` by a redirection may be missed, never a false
//! block), and the common form — flags before the redirect, e.g.
//! `rg -r foo 2>/dev/null` — is unaffected (the `-r` is seen first).

/// `NAME=value` assignment prefix: NAME starts with a letter/underscore and is
/// otherwise all word chars. Mirrors the zsh test
/// `[[ $pre == [A-Za-z_]* && $pre != *[^A-Za-z0-9_]* ]]`. Shared by the guard's
/// head resolution and the lexer's command-position tracking — an assignment
/// prefix keeps command position (`X=1 (rg ...)` -> `(` opens a subshell).
pub(crate) fn is_assignment(t: &str) -> bool {
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

/// Whether the token after `word` is still in command position (so a `(` there
/// opens a subshell). These are zsh reserved words that expect a command to
/// follow, the `nocorrect` precommand modifier, and assignment prefixes. This is
/// zsh's tokenizer grammar and is DISTINCT from the guard's wrapper set: `time`
/// keeps command position here, but `sudo` consumes it.
///
/// Deliberately excluded (special paren / arity / definition syntax — not
/// modeled, and `rg -r` never appears there): `repeat N`, `for VAR (list)`,
/// `case PAT)`, `function name`, `coproc`, `[[ ... ]]`, arithmetic `(( ... ))`.
fn preserves_cmd_pos(word: &str) -> bool {
    matches!(
        word,
        "time"
            | "nocorrect"
            | "do"
            | "then"
            | "else"
            | "elif"
            | "if"
            | "while"
            | "until"
            | "!"
            | "done"
            | "fi"
            | "esac"
            | "{"
            | "}"
    ) || is_assignment(word)
}

/// Split `src` into shell-ish tokens (see module docs for the exact rules).
pub fn tokenize(src: &str) -> Vec<String> {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut toks: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    // Command-position tracking — needed only to tokenize `(` / `)` like zsh.
    // `depth` counts open command-position subshells so a closing `)` knows it
    // returns to argument context, while a stray `)` starts a new command.
    let mut cmd_pos = true;
    let mut depth: u32 = 0;

    while i < n {
        let c = chars[i];
        match c {
            // space/tab end a word; a newline additionally yields a `;` token.
            // `\r` is deliberately absent: zsh `(z)` keeps it inside the token as
            // an ordinary char (see module docs), so it falls through to `_`.
            ' ' | '\t' => {
                finish_word(&mut toks, &mut cur, &mut cmd_pos);
                i += 1;
            }
            '\n' => {
                finish_word(&mut toks, &mut cur, &mut cmd_pos);
                toks.push(";".to_string());
                cmd_pos = true;
                i += 1;
            }
            // backslash escapes the next char; keep both verbatim in the word
            '\\' => {
                cur.push('\\');
                if i + 1 < n {
                    cur.push(chars[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            // single quotes: verbatim, no escapes inside
            '\'' => {
                cur.push('\'');
                i += 1;
                while i < n && chars[i] != '\'' {
                    cur.push(chars[i]);
                    i += 1;
                }
                if i < n {
                    cur.push('\'');
                    i += 1;
                }
            }
            // double quotes: verbatim; backslash escapes the next char
            '"' => {
                cur.push('"');
                i += 1;
                while i < n && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < n {
                        cur.push('\\');
                        cur.push(chars[i + 1]);
                        i += 2;
                    } else {
                        cur.push(chars[i]);
                        i += 1;
                    }
                }
                if i < n {
                    cur.push('"');
                    i += 1;
                }
            }
            // backtick command substitution: stays inside the word
            '`' => {
                cur.push('`');
                i += 1;
                while i < n && chars[i] != '`' {
                    cur.push(chars[i]);
                    i += 1;
                }
                if i < n {
                    cur.push('`');
                    i += 1;
                }
            }
            // $( ... ) substitution or ${ ... } expansion, balanced — kept whole
            '$' if i + 1 < n && (chars[i + 1] == '(' || chars[i + 1] == '{') => {
                let open = chars[i + 1];
                let close = if open == '(' { ')' } else { '}' };
                cur.push('$');
                cur.push(open);
                i = scan_balanced(&chars, i + 2, open, close, &mut cur);
            }
            // `(` in command position opens a subshell -> its own token. As an
            // argument (mid-word, or after a command word) it is a glob group
            // glued into the current word, matching zsh `(z)`.
            '(' if cur.is_empty() && cmd_pos => {
                // command position continues inside the subshell, so `cmd_pos`
                // is left unchanged (the match guard already required it true).
                toks.push("(".to_string());
                depth = depth.saturating_add(1); // overflow-safe (decrement already is)
                i += 1;
            }
            '(' => {
                cur.push('(');
                i = scan_balanced(&chars, i + 1, '(', ')', &mut cur);
            }
            // `)` always ends the word and is its own token. Closing a subshell
            // returns to argument context (so `(a) (b)` keeps `(b)` glued); a
            // stray `)` starts a new command (so `)rg -r x` blocks) — matching zsh.
            ')' => {
                finish_word(&mut toks, &mut cur, &mut cmd_pos);
                toks.push(")".to_string());
                if depth > 0 {
                    depth -= 1;
                    cmd_pos = false;
                } else {
                    cmd_pos = true;
                }
                i += 1;
            }
            // operator families: prefer a two-char operator, else the single
            // char. two-char set: || |& && ;; ;& ;|
            '|' | '&' | ';' => {
                finish_word(&mut toks, &mut cur, &mut cmd_pos);
                let two = if i + 1 < n {
                    match (c, chars[i + 1]) {
                        ('|', '|') => Some("||"),
                        ('|', '&') => Some("|&"),
                        ('&', '&') => Some("&&"),
                        (';', ';') => Some(";;"),
                        (';', '&') => Some(";&"),
                        (';', '|') => Some(";|"),
                        _ => None,
                    }
                } else {
                    None
                };
                match two {
                    Some(op) => {
                        toks.push(op.to_string());
                        i += 2;
                    }
                    None => {
                        toks.push(c.to_string());
                        i += 1;
                    }
                }
                cmd_pos = true; // an operator starts a new command
            }
            _ => {
                cur.push(c);
                i += 1;
            }
        }
    }
    finish_word(&mut toks, &mut cur, &mut cmd_pos);
    toks
}

/// Push the pending word (if any) and update command position from it. A word
/// can only KEEP command position (a reserved word / assignment that itself sits
/// in command position); a regular word, or any word already in argument
/// position, latches it off until the next operator. So `cat time (rg -r x)`
/// glues the group — the `time` is cat's argument, not a fresh command.
fn finish_word(toks: &mut Vec<String>, cur: &mut String, cmd_pos: &mut bool) {
    if !cur.is_empty() {
        let w = std::mem::take(cur);
        *cmd_pos = *cmd_pos && preserves_cmd_pos(&w);
        toks.push(w);
    }
}

/// Copy a balanced `open`..`close` span (starting just past the opener) into
/// `cur`, returning the index just past the closer. Shared by `$( ... )`,
/// `${ ... }`, and argument-position `( ... )` glob groups. An unterminated span
/// runs to end-of-input (fail-open: the lexer just keeps the rest in the word).
fn scan_balanced(chars: &[char], mut i: usize, open: char, close: char, cur: &mut String) -> usize {
    let mut depth = 1;
    while i < chars.len() && depth > 0 {
        let d = chars[i];
        if d == open {
            depth += 1;
        } else if d == close {
            depth -= 1;
        }
        cur.push(d);
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::tokenize;

    fn t(s: &str) -> Vec<String> {
        tokenize(s)
    }

    // Each expectation below is the exact `${(z)}` output captured from zsh.
    #[test]
    fn matches_zsh_z_output() {
        assert_eq!(t("rg -r foo"), ["rg", "-r", "foo"]);
        assert_eq!(t("rg '-r' x"), ["rg", "'-r'", "x"]);
        assert_eq!(t("rg \"-r\" x"), ["rg", "\"-r\"", "x"]);
        assert_eq!(t("rg $(grep -r x) y"), ["rg", "$(grep -r x)", "y"]);
        assert_eq!(t("rg `grep -r x` y"), ["rg", "`grep -r x`", "y"]);
        assert_eq!(t("cat a | rg -r b"), ["cat", "a", "|", "rg", "-r", "b"]);
        assert_eq!(t("rg foo | sort -r"), ["rg", "foo", "|", "sort", "-r"]);
        assert_eq!(t("rg foo && rm -r d"), ["rg", "foo", "&&", "rm", "-r", "d"]);
        assert_eq!(t("rg foo; rm -r d"), ["rg", "foo", ";", "rm", "-r", "d"]);
        assert_eq!(t("rg foo\nrm -r d"), ["rg", "foo", ";", "rm", "-r", "d"]);
        assert_eq!(t("rg -- -r"), ["rg", "--", "-r"]);
        assert_eq!(t("rg -rn foo"), ["rg", "-rn", "foo"]);
        assert_eq!(t("env X=1 rg -r x"), ["env", "X=1", "rg", "-r", "x"]);
        assert_eq!(t("/usr/bin/rg -r x"), ["/usr/bin/rg", "-r", "x"]);
        assert_eq!(t("rg --replace=x p"), ["rg", "--replace=x", "p"]);
        assert_eq!(t("(rg -r x)"), ["(", "rg", "-r", "x", ")"]);
        assert_eq!(t("{ rg -r x; }"), ["{", "rg", "-r", "x", ";", "}"]);
        // a carriage return is kept inside the token, never a separator:
        // `rg\r-r` is ONE token (head != "rg" -> allow), matching zsh `(z)`.
        assert_eq!(t("rg\r-r"), ["rg\r-r"]);
        assert_eq!(t("rg fo\ro -r bar"), ["rg", "fo\ro", "-r", "bar"]);
        // `(` is position-dependent: a subshell at command position splits, but
        // an argument-position `(` is a glob group glued into one token.
        assert_eq!(t("foo (rg -r x)"), ["foo", "(rg -r x)"]);
        assert_eq!(t("sudo (rg -r x"), ["sudo", "(rg -r x"]);
        assert_eq!(t("X=1 (rg -r x"), ["X=1", "(", "rg", "-r", "x"]);
        assert_eq!(t("time (rg -r x)"), ["time", "(", "rg", "-r", "x", ")"]);
        assert_eq!(t("echo (a) (rg -r x"), ["echo", "(a)", "(rg -r x"]);
        assert_eq!(t("rg foo)"), ["rg", "foo", ")"]);
        // closing a subshell returns to argument context: the second group glues
        assert_eq!(t("(bar) (rg -r x)"), ["(", "bar", ")", "(rg -r x)"]);
        assert_eq!(
            t("(rg -r x) (rg foo)"),
            ["(", "rg", "-r", "x", ")", "(rg foo)"]
        );
        // a stray `)` (no open subshell) starts a new command
        assert_eq!(t(")rg -r x"), [")", "rg", "-r", "x"]);
    }
}

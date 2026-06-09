//! Quote-aware tokenizer matching zsh `${(z)}` closely enough to reproduce the
//! guard's block/allow verdicts. Behaviour captured empirically from `(z)` and
//! enforced by a differential fuzzer (`scripts/fuzz.zsh`).
//!
//! Captured rules:
//! - single/double quotes are RETAINED in the token, so a quoted `-r` is not bare
//! - `$(...)`, `${...}`, and backtick spans stay inside one word token
//! - command operators split into their own tokens, even without surrounding
//!   whitespace: `|` `||` `|&` `&` `&&` `;` `;;` `;&` `;|` `(` `)`
//! - a newline becomes a `;` token
//!
//! `{` / `}` are NOT split here — in zsh they are reserved words only when
//! whitespace-delimited, so they already surface as standalone tokens and the
//! state machine treats them as operators.

/// Split `src` into shell-ish tokens (see module docs for the exact rules).
pub fn tokenize(src: &str) -> Vec<String> {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut toks: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut i = 0;

    while i < n {
        let c = chars[i];
        match c {
            // whitespace ends a word; a newline additionally yields a `;` token
            ' ' | '\t' | '\r' => {
                push_word(&mut toks, &mut cur);
                i += 1;
            }
            '\n' => {
                push_word(&mut toks, &mut cur);
                toks.push(";".to_string());
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
            // $( ... ) command substitution, balanced — stays inside the word
            '$' if i + 1 < n && chars[i + 1] == '(' => {
                cur.push('$');
                cur.push('(');
                i += 2;
                let mut depth = 1;
                while i < n && depth > 0 {
                    let d = chars[i];
                    if d == '(' {
                        depth += 1;
                    } else if d == ')' {
                        depth -= 1;
                    }
                    cur.push(d);
                    i += 1;
                }
            }
            // ${ ... } parameter expansion, balanced — stays inside the word
            '$' if i + 1 < n && chars[i + 1] == '{' => {
                cur.push('$');
                cur.push('{');
                i += 2;
                let mut depth = 1;
                while i < n && depth > 0 {
                    let d = chars[i];
                    if d == '{' {
                        depth += 1;
                    } else if d == '}' {
                        depth -= 1;
                    }
                    cur.push(d);
                    i += 1;
                }
            }
            // pipe family: | || |&
            '|' => {
                push_word(&mut toks, &mut cur);
                if i + 1 < n && chars[i + 1] == '|' {
                    toks.push("||".to_string());
                    i += 2;
                } else if i + 1 < n && chars[i + 1] == '&' {
                    toks.push("|&".to_string());
                    i += 2;
                } else {
                    toks.push("|".to_string());
                    i += 1;
                }
            }
            // ampersand family: && &
            '&' => {
                push_word(&mut toks, &mut cur);
                if i + 1 < n && chars[i + 1] == '&' {
                    toks.push("&&".to_string());
                    i += 2;
                } else {
                    toks.push("&".to_string());
                    i += 1;
                }
            }
            // semicolon family: ;; ;& ;| ;
            ';' => {
                push_word(&mut toks, &mut cur);
                if i + 1 < n && chars[i + 1] == ';' {
                    toks.push(";;".to_string());
                    i += 2;
                } else if i + 1 < n && chars[i + 1] == '&' {
                    toks.push(";&".to_string());
                    i += 2;
                } else if i + 1 < n && chars[i + 1] == '|' {
                    toks.push(";|".to_string());
                    i += 2;
                } else {
                    toks.push(";".to_string());
                    i += 1;
                }
            }
            '(' => {
                push_word(&mut toks, &mut cur);
                toks.push("(".to_string());
                i += 1;
            }
            ')' => {
                push_word(&mut toks, &mut cur);
                toks.push(")".to_string());
                i += 1;
            }
            _ => {
                cur.push(c);
                i += 1;
            }
        }
    }
    push_word(&mut toks, &mut cur);
    toks
}

fn push_word(toks: &mut Vec<String>, cur: &mut String) {
    if !cur.is_empty() {
        toks.push(std::mem::take(cur));
    }
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
    }
}

// SPDX-License-Identifier: Apache-2.0
//! Quoting of the bridge command for `sh -c` and reading it back (REQ-117).
//!
//! The status line command that the setup writes is a line of text that Claude Code hands to a
//! shell. On Linux and macOS that is `sh -c`, so a path with a space or another special character
//! has to be quoted the way `sh` reads it. [`first_word`] reads a command the way a shell does
//! (single quotes, double quotes, `\'` as the generator writes it), so the setup can recognise its
//! own command again whatever form it has.
//!
//! This file has no dependency on the rest of the program: it is compiled and tested on every
//! system, and the integration tests include it with `#[path]` to check it against a real shell.

/// Characters that need no quoting in a word for `sh`.
fn is_safe(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_-./:@%+=,".contains(c)
}

/// `text` as one word for `sh`: unchanged if it has only safe characters, otherwise in single
/// quotes, with a single quote inside written as `'\''`.
pub fn sh_quote(text: &str) -> String {
    if !text.is_empty() && text.chars().all(is_safe) {
        return text.to_owned();
    }
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The first word of `command`, read the way a shell reads it, and the text after it.
///
/// Leading white space is skipped. Inside single or double quotes everything is taken as it is;
/// outside quotes `\'` is a single quote and every other backslash stays a backslash (an old
/// Windows path such as `C:\bin\usage-cockpit.exe` is one word). `None` if a quote is not closed.
/// A closing quote that is directly followed by more text continues the same word, as in a shell.
pub fn first_word(command: &str) -> Option<(String, &str)> {
    let command = command.trim_start();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        match quote {
            Some(open) if c == open => quote = None,
            Some(_) => word.push(c),
            None => match c {
                '\'' | '"' => quote = Some(c),
                '\\' if matches!(chars.peek(), Some((_, '\''))) => {
                    chars.next();
                    word.push('\'');
                }
                c if c.is_whitespace() => return Some((word, &command[at..])),
                other => word.push(other),
            },
        }
    }
    quote.is_none().then_some((word, ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_117_safe_text_stays_as_it_is() {
        assert_eq!(
            sh_quote("/opt/tools/usage-cockpit"),
            "/opt/tools/usage-cockpit"
        );
        assert_eq!(
            sh_quote("C:/Users/me/usage-cockpit.exe"),
            "C:/Users/me/usage-cockpit.exe"
        );
    }

    #[test]
    fn req_117_special_characters_are_quoted() {
        assert_eq!(sh_quote("/opt/my tools/x"), "'/opt/my tools/x'");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
        assert_eq!(sh_quote("a$b"), "'a$b'");
        assert_eq!(sh_quote(""), "''");
        assert_eq!(sh_quote("a\"b"), "'a\"b'");
        assert_eq!(sh_quote("a\\b"), "'a\\b'");
    }

    #[test]
    fn req_117_what_is_quoted_is_read_back() {
        for text in [
            "/opt/tools/usage-cockpit",
            "/opt/my tools/usage-cockpit",
            "/it's/a $HOME/`x`/\"y\"/usage-cockpit",
            "/tab\there/usage-cockpit",
            "/ünï/cödé/usage-cockpit",
            "/back\\slash/usage-cockpit",
            "",
        ] {
            let command = format!("{} bridge", sh_quote(text));
            let (word, rest) = first_word(&command).expect(&command);
            assert_eq!(word, text, "{command}");
            assert_eq!(rest, " bridge", "{command}");
        }
    }

    #[test]
    fn req_117_first_word_reads_the_forms_of_the_command() {
        let word = |command| first_word(command).map(|(w, r)| (w, r.to_owned()));
        assert_eq!(
            word("/opt/tools/usage-cockpit bridge"),
            Some(("/opt/tools/usage-cockpit".into(), " bridge".into()))
        );
        assert_eq!(
            word("  \"C:/My Tools/usage-cockpit.exe\" bridge"),
            Some(("C:/My Tools/usage-cockpit.exe".into(), " bridge".into()))
        );
        assert_eq!(
            word("C:\\bin\\Usage-Cockpit.EXE bridge"),
            Some(("C:\\bin\\Usage-Cockpit.EXE".into(), " bridge".into()))
        );
        // a closing quote that is followed by text continues the word, as in a shell
        assert_eq!(
            word("\"C:/x/usage-cockpit.exe\"bridge"),
            Some(("C:/x/usage-cockpit.exebridge".into(), String::new()))
        );
        assert_eq!(word("'unterminated bridge"), None);
        assert_eq!(word("\"unterminated bridge"), None);
        assert_eq!(word("alone"), Some(("alone".into(), String::new())));
        assert_eq!(word(""), Some((String::new(), String::new())));
    }
}

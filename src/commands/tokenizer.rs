//! Tokenizer for command argument parsing.
//!
//! Provides robust parsing of command arguments with support for:
//! - Quoted strings (single and double quotes)
//! - Escape sequences within quotes
//! - Key=value pairs
//! - Flags (--flag or -f)

/// A token parsed from command input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// A plain word (unquoted argument).
    Word(String),
    /// A key=value pair.
    KeyValue { key: String, value: String },
    /// A long flag (--flag).
    LongFlag(String),
    /// A short flag (-f).
    ShortFlag(char),
}

#[allow(dead_code)]
impl Token {
    /// Returns the token as a word if it is one.
    pub fn as_word(&self) -> Option<&str> {
        match self {
            Token::Word(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the key-value pair if this is a KeyValue token.
    pub fn as_key_value(&self) -> Option<(&str, &str)> {
        match self {
            Token::KeyValue { key, value } => Some((key, value)),
            _ => None,
        }
    }

    /// Returns true if this is a long flag with the given name.
    pub fn is_long_flag(&self, name: &str) -> bool {
        matches!(self, Token::LongFlag(n) if n == name)
    }

    /// Returns true if this is a short flag with the given character.
    pub fn is_short_flag(&self, c: char) -> bool {
        matches!(self, Token::ShortFlag(ch) if *ch == c)
    }
}

/// Tokenizes a command argument string.
///
/// Handles:
/// - Whitespace-separated tokens
/// - Double-quoted strings: `"hello world"` → `hello world`
/// - Single-quoted strings: `'hello world'` → `hello world`
/// - Escape sequences in quotes: `"say \"hi\""` → `say "hi"`
/// - Key=value pairs: `host=localhost` → KeyValue { key: "host", value: "localhost" }
/// - Quoted values: `password="my secret"` → KeyValue { key: "password", value: "my secret" }
/// - Long flags: `--test` → LongFlag("test")
/// - Short flags: `-t` → ShortFlag('t')
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        // Skip whitespace
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        // Check for flags
        if c == '-' {
            chars.next();
            if let Some(&next) = chars.peek() {
                if next == '-' {
                    // Long flag: --flag
                    chars.next();
                    let flag_name = collect_word(&mut chars);
                    if !flag_name.is_empty() {
                        tokens.push(Token::LongFlag(flag_name));
                    }
                    continue;
                } else if next.is_alphabetic() {
                    // Short flag: -f
                    chars.next();
                    tokens.push(Token::ShortFlag(next));
                    continue;
                }
            }
            // Just a dash, treat as word
            let mut word = String::from("-");
            word.push_str(&collect_word(&mut chars));
            tokens.push(Token::Word(word));
            continue;
        }

        // Collect a word or key=value
        let word = collect_word_or_quoted(&mut chars);
        if word.is_empty() {
            continue;
        }

        // Check if it's a key=value pair
        if let Some(eq_pos) = word.find('=') {
            let key = word[..eq_pos].to_string();
            let value_part = &word[eq_pos + 1..];

            // Value might be quoted or continue after the =
            let value = if value_part.is_empty() {
                // Check if next char is a quote
                if let Some(&quote @ ('"' | '\'')) = chars.peek() {
                    chars.next();
                    collect_quoted(&mut chars, quote)
                } else {
                    String::new()
                }
            } else if value_part.starts_with('"') || value_part.starts_with('\'') {
                // Value started with quote but was collected as part of word
                // This shouldn't happen with our current logic, but handle it
                value_part
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string()
            } else {
                value_part.to_string()
            };

            tokens.push(Token::KeyValue { key, value });
        } else {
            tokens.push(Token::Word(word));
        }
    }

    tokens
}

/// Collects characters until whitespace or end of input.
fn collect_word(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut word = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            break;
        }
        chars.next();
        word.push(c);
    }
    word
}

/// Collects a word, handling quoted strings.
fn collect_word_or_quoted(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut result = String::new();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            break;
        }

        if c == '"' || c == '\'' {
            chars.next();
            let quoted = collect_quoted(chars, c);
            result.push_str(&quoted);
            continue;
        }

        // Check for key=value with quoted value
        if c == '=' {
            result.push(c);
            chars.next();
            // Check if value is quoted
            if let Some(&quote @ ('"' | '\'')) = chars.peek() {
                chars.next();
                let quoted = collect_quoted(chars, quote);
                result.push_str(&quoted);
                break; // End of this token
            }
            continue;
        }

        chars.next();
        result.push(c);
    }

    result
}

/// Collects characters inside quotes, handling escape sequences.
fn collect_quoted(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, quote: char) -> String {
    let mut result = String::new();
    let mut escaped = false;

    while let Some(&c) = chars.peek() {
        chars.next();

        if escaped {
            // Handle escape sequences
            match c {
                'n' => result.push('\n'),
                't' => result.push('\t'),
                'r' => result.push('\r'),
                '\\' => result.push('\\'),
                '"' => result.push('"'),
                '\'' => result.push('\''),
                _ => {
                    // Unknown escape, keep as-is
                    result.push('\\');
                    result.push(c);
                }
            }
            escaped = false;
            continue;
        }

        if c == '\\' {
            escaped = true;
            continue;
        }

        if c == quote {
            break; // End of quoted string
        }

        result.push(c);
    }

    result
}

/// Parse error with context for helpful error messages.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// The command that failed to parse.
    pub command: String,
    /// Error message describing what went wrong.
    pub message: String,
    /// Optional hint for how to fix the error.
    pub hint: Option<String>,
}

#[allow(dead_code)]
impl ParseError {
    /// Creates a new parse error.
    pub fn new(command: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            message: message.into(),
            hint: None,
        }
    }

    /// Adds a hint to the error.
    pub fn with_hint(self, hint: impl Into<String>) -> Self {
        Self {
            hint: Some(hint.into()),
            ..self
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.command, self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, "\nHint: {}", hint)?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_words() {
        let tokens = tokenize("hello world");
        assert_eq!(
            tokens,
            vec![
                Token::Word("hello".to_string()),
                Token::Word("world".to_string())
            ]
        );
    }

    #[test]
    fn test_double_quoted_string() {
        let tokens = tokenize("name=\"John Doe\"");
        assert_eq!(
            tokens,
            vec![Token::KeyValue {
                key: "name".to_string(),
                value: "John Doe".to_string()
            }]
        );
    }

    #[test]
    fn test_single_quoted_string() {
        let tokens = tokenize("name='John Doe'");
        assert_eq!(
            tokens,
            vec![Token::KeyValue {
                key: "name".to_string(),
                value: "John Doe".to_string()
            }]
        );
    }

    #[test]
    fn test_escaped_quotes() {
        let tokens = tokenize(r#"msg="say \"hello\"""#);
        assert_eq!(
            tokens,
            vec![Token::KeyValue {
                key: "msg".to_string(),
                value: "say \"hello\"".to_string()
            }]
        );
    }

    #[test]
    fn test_key_value_unquoted() {
        let tokens = tokenize("host=localhost port=5432");
        assert_eq!(
            tokens,
            vec![
                Token::KeyValue {
                    key: "host".to_string(),
                    value: "localhost".to_string()
                },
                Token::KeyValue {
                    key: "port".to_string(),
                    value: "5432".to_string()
                }
            ]
        );
    }

    #[test]
    fn test_long_flag() {
        let tokens = tokenize("--test --verbose");
        assert_eq!(
            tokens,
            vec![
                Token::LongFlag("test".to_string()),
                Token::LongFlag("verbose".to_string())
            ]
        );
    }

    #[test]
    fn test_short_flag() {
        let tokens = tokenize("-t -v");
        assert_eq!(tokens, vec![Token::ShortFlag('t'), Token::ShortFlag('v')]);
    }

    #[test]
    fn test_mixed_tokens() {
        let tokens = tokenize("mydb host=localhost --test -v");
        assert_eq!(
            tokens,
            vec![
                Token::Word("mydb".to_string()),
                Token::KeyValue {
                    key: "host".to_string(),
                    value: "localhost".to_string()
                },
                Token::LongFlag("test".to_string()),
                Token::ShortFlag('v')
            ]
        );
    }

    #[test]
    fn test_password_with_spaces() {
        let tokens = tokenize("password=\"my secret password\"");
        assert_eq!(
            tokens,
            vec![Token::KeyValue {
                key: "password".to_string(),
                value: "my secret password".to_string()
            }]
        );
    }

    #[test]
    fn test_password_with_special_chars() {
        let tokens = tokenize("password=\"p@ss=word!\"");
        assert_eq!(
            tokens,
            vec![Token::KeyValue {
                key: "password".to_string(),
                value: "p@ss=word!".to_string()
            }]
        );
    }

    #[test]
    fn test_empty_value() {
        let tokens = tokenize("name=");
        assert_eq!(
            tokens,
            vec![Token::KeyValue {
                key: "name".to_string(),
                value: "".to_string()
            }]
        );
    }

    #[test]
    fn test_quoted_word_standalone() {
        let tokens = tokenize("\"hello world\"");
        assert_eq!(tokens, vec![Token::Word("hello world".to_string())]);
    }

    #[test]
    fn test_escape_sequences() {
        let tokens = tokenize(r#""line1\nline2\ttab""#);
        assert_eq!(tokens, vec![Token::Word("line1\nline2\ttab".to_string())]);
    }

    #[test]
    fn test_conn_add_realistic() {
        let tokens = tokenize("mydb host=localhost port=5432 database=mydb user=postgres password=\"my secret\" --test");
        assert_eq!(
            tokens,
            vec![
                Token::Word("mydb".to_string()),
                Token::KeyValue {
                    key: "host".to_string(),
                    value: "localhost".to_string()
                },
                Token::KeyValue {
                    key: "port".to_string(),
                    value: "5432".to_string()
                },
                Token::KeyValue {
                    key: "database".to_string(),
                    value: "mydb".to_string()
                },
                Token::KeyValue {
                    key: "user".to_string(),
                    value: "postgres".to_string()
                },
                Token::KeyValue {
                    key: "password".to_string(),
                    value: "my secret".to_string()
                },
                Token::LongFlag("test".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_error_display() {
        let err = ParseError::new("/conn add", "Missing required argument: name")
            .with_hint("Usage: /conn add <name> host=<host> ...");
        let display = err.to_string();
        assert!(display.contains("/conn add"));
        assert!(display.contains("Missing required argument"));
        assert!(display.contains("Hint:"));
    }

    #[test]
    fn test_token_methods() {
        let word = Token::Word("test".to_string());
        assert_eq!(word.as_word(), Some("test"));
        assert_eq!(word.as_key_value(), None);

        let kv = Token::KeyValue {
            key: "host".to_string(),
            value: "localhost".to_string(),
        };
        assert_eq!(kv.as_key_value(), Some(("host", "localhost")));
        assert_eq!(kv.as_word(), None);

        let long_flag = Token::LongFlag("test".to_string());
        assert!(long_flag.is_long_flag("test"));
        assert!(!long_flag.is_long_flag("other"));

        let short_flag = Token::ShortFlag('t');
        assert!(short_flag.is_short_flag('t'));
        assert!(!short_flag.is_short_flag('v'));
    }
}

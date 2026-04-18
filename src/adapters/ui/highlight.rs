//! Minimal hand-rolled Solidity syntax highlighter.
//!
//! Returns a list of `ratatui::text::Line` whose spans carry semantic
//! colour runs. `syntect` remains the long-term preferred backend
//! once we bundle a Solidity `.sublime-syntax`; see
//! `plan/7-contract-detail.md` §12.5.4 for the decision log.
//!
//! The tokeniser is deliberately permissive: anything it cannot
//! classify falls through as a plain-text span. That keeps the
//! highlighter robust to experimental Solidity syntax (Yul blocks,
//! user-defined operators, `using for` directives, etc.) without
//! crashing — we just render those parts in the default colour.
//!
//! # Highlight classes
//!
//! * `Keyword` — language keywords.
//! * `Type` — primitive ABI types (`uint`, `int`, `address`,
//!   `bool`, `bytes`, `bytesN`, `uintN`, `intN`, `string`).
//! * `StringLit` — double-quoted strings (including `hex"..."`).
//! * `Number` — numeric literals with optional `_` separators and
//!   `wei` / `ether` / `gwei` suffixes.
//! * `Comment` — `//` line comments and `/* */` block comments.
//!
//! The highlighter is a pure function: no allocation per character,
//! one `Span` per contiguous classified run.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Semantic classification of a lexeme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Plain,
    Keyword,
    Type,
    StringLit,
    Number,
    Comment,
}

impl Class {
    fn style(self) -> Style {
        match self {
            Class::Plain => Style::default(),
            Class::Keyword => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            Class::Type => Style::default().fg(Color::LightBlue),
            Class::StringLit => Style::default().fg(Color::LightGreen),
            Class::Number => Style::default().fg(Color::Yellow),
            Class::Comment => Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        }
    }
}

/// One classified token. `text` is an owned slice so the returned
/// `Line<'static>` can outlive the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub class: Class,
}

impl Token {
    fn new(text: impl Into<String>, class: Class) -> Self {
        Self {
            text: text.into(),
            class,
        }
    }
}

const KEYWORDS: &[&str] = &[
    "abstract",
    "after",
    "anonymous",
    "as",
    "assembly",
    "break",
    "calldata",
    "case",
    "catch",
    "constant",
    "constructor",
    "continue",
    "contract",
    "default",
    "delete",
    "do",
    "else",
    "emit",
    "enum",
    "event",
    "external",
    "fallback",
    "for",
    "function",
    "immutable",
    "import",
    "indexed",
    "interface",
    "internal",
    "is",
    "library",
    "mapping",
    "memory",
    "modifier",
    "new",
    "override",
    "payable",
    "pragma",
    "private",
    "public",
    "pure",
    "receive",
    "require",
    "return",
    "returns",
    "revert",
    "storage",
    "struct",
    "switch",
    "throw",
    "try",
    "type",
    "unchecked",
    "using",
    "view",
    "virtual",
    "while",
];

const RESERVED_LITERALS: &[&str] = &["true", "false"];

const NAMED_TYPES: &[&str] = &["address", "bool", "string", "bytes"];

const NUMERIC_SUFFIXES: &[&str] = &[
    "wei", "gwei", "ether", "seconds", "minutes", "hours", "days", "weeks",
];

fn is_ident_start(c: char) -> bool {
    c == '_' || c == '$' || c.is_ascii_alphabetic()
}

fn is_ident_cont(c: char) -> bool {
    c == '_' || c == '$' || c.is_ascii_alphanumeric()
}

fn classify_ident(ident: &str) -> Class {
    if KEYWORDS.binary_search(&ident).is_ok() || RESERVED_LITERALS.contains(&ident) {
        return Class::Keyword;
    }
    if NAMED_TYPES.contains(&ident) {
        return Class::Type;
    }
    // uintN / intN / bytesN where N is an integer. Accept bare
    // `uint`, `int`, `bytes` too.
    for prefix in ["uint", "int", "bytes"] {
        if let Some(rest) = ident.strip_prefix(prefix)
            && (rest.is_empty() || rest.chars().all(|c| c.is_ascii_digit()))
        {
            return Class::Type;
        }
    }
    Class::Plain
}

/// Tokenise one line of Solidity source. `in_block_comment` is the
/// lexer state carried across lines; it tracks whether the line
/// opens (or continues) inside a `/* */` block.
fn tokenise_line(line: &str, in_block_comment: &mut bool) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Continuing a multi-line block comment.
        if *in_block_comment {
            let start = i;
            while i < bytes.len() {
                if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    *in_block_comment = false;
                    break;
                }
                i += 1;
            }
            out.push(Token::new(&line[start..i], Class::Comment));
            continue;
        }

        let c = bytes[i] as char;

        // `//` line comment — consumes to end of line.
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            out.push(Token::new(&line[i..], Class::Comment));
            i = bytes.len();
            continue;
        }

        // `/* ... */` block comment — may span lines.
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            *in_block_comment = true;
            while i < bytes.len() {
                if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    *in_block_comment = false;
                    break;
                }
                i += 1;
            }
            out.push(Token::new(&line[start..i], Class::Comment));
            continue;
        }

        // Double-quoted string. Solidity supports hex"..." and
        // unicode"..." prefixes; we treat the prefix as part of
        // the string if it is an ident immediately followed by a
        // quote.
        if c == '"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i < bytes.len() {
                i += 1;
            }
            out.push(Token::new(&line[start..i], Class::StringLit));
            continue;
        }

        // Identifier / keyword / type.
        if is_ident_start(c) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_cont(bytes[i] as char) {
                i += 1;
            }
            let ident = &line[start..i];
            out.push(Token::new(ident, classify_ident(ident)));
            continue;
        }

        // Numeric literal.
        if c.is_ascii_digit() {
            let start = i;
            // `0x...` or `0X...`
            if bytes[i] == b'0'
                && i + 1 < bytes.len()
                && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X')
            {
                i += 2;
                while i < bytes.len() && (bytes[i] as char).is_ascii_hexdigit() {
                    i += 1;
                }
            } else {
                while i < bytes.len() && ((bytes[i] as char).is_ascii_digit() || bytes[i] == b'_') {
                    i += 1;
                }
                // Decimal point.
                if i < bytes.len() && bytes[i] == b'.' {
                    i += 1;
                    while i < bytes.len()
                        && ((bytes[i] as char).is_ascii_digit() || bytes[i] == b'_')
                    {
                        i += 1;
                    }
                }
                // Exponent (e10, E-2, ...).
                if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                    i += 1;
                    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                        i += 1;
                    }
                    while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            out.push(Token::new(&line[start..i], Class::Number));

            // Optional unit suffix (`1 ether`, `10 gwei`, …) — this
            // is a separate identifier after whitespace, so just
            // skip the whitespace and see whether it matches. The
            // tokeniser would classify a bare `ether` as `Plain`,
            // so give it a special promotion here.
            let ws_start = i;
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            if i > ws_start {
                let ws = &line[ws_start..i];
                out.push(Token::new(ws, Class::Plain));
            }
            if i < bytes.len() && is_ident_start(bytes[i] as char) {
                let id_start = i;
                while i < bytes.len() && is_ident_cont(bytes[i] as char) {
                    i += 1;
                }
                let ident = &line[id_start..i];
                if NUMERIC_SUFFIXES.contains(&ident) {
                    out.push(Token::new(ident, Class::Number));
                } else {
                    out.push(Token::new(ident, classify_ident(ident)));
                }
            }
            continue;
        }

        // Anything else: consume a contiguous run of unclassified
        // characters (whitespace, punctuation, operators) as a
        // single plain span.
        let start = i;
        while i < bytes.len() {
            let cc = bytes[i] as char;
            if cc == '/' && i + 1 < bytes.len() && (bytes[i + 1] == b'/' || bytes[i + 1] == b'*') {
                break;
            }
            if cc == '"' || is_ident_start(cc) || cc.is_ascii_digit() {
                break;
            }
            i += 1;
        }
        out.push(Token::new(&line[start..i], Class::Plain));
    }

    out
}

/// Render `source` as a `Vec<Line<'static>>` ready to hand to a
/// [`ratatui::widgets::Paragraph`] through `Text::from`.
#[must_use]
pub fn highlight_solidity(source: &str) -> Vec<Line<'static>> {
    // Keyword list is declared in source order but `binary_search`
    // in `classify_ident` needs it sorted; this asserts it at
    // runtime under debug once the highlighter runs for the first
    // time. Cheap and keeps the constant table next to the grammar.
    debug_assert!(KEYWORDS.windows(2).all(|w| w[0] < w[1]));

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_block_comment = false;
    for raw in source.split('\n') {
        // `split('\n')` keeps the trailing carriage return on
        // Windows-style lines; strip it for display.
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let tokens = tokenise_line(line, &mut in_block_comment);
        let spans: Vec<Span<'static>> = tokens
            .into_iter()
            .map(|t| Span::styled(t.text, t.class.style()))
            .collect();
        lines.push(Line::from(spans));
    }
    lines
}

/// Test-only helper: flatten the line into a list of `(class,
/// text)` pairs so assertions can match structure without caring
/// about the concrete `Style`.
#[doc(hidden)]
#[must_use]
pub fn highlight_for_tests(source: &str) -> Vec<Vec<(Class, String)>> {
    let mut lines: Vec<Vec<(Class, String)>> = Vec::new();
    let mut in_block_comment = false;
    for raw in source.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let tokens = tokenise_line(line, &mut in_block_comment);
        lines.push(tokens.into_iter().map(|t| (t.class, t.text)).collect());
    }
    lines
}

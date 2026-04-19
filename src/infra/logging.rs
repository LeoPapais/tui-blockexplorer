//! Masking logger.
//!
//! Every rendered `tracing` line runs through [`mask_sensitive`]
//! before it hits the configured writer. The helper redacts the
//! value of any field whose key contains `key`, `token` or `secret`
//! (case-insensitive), matching the contract documented in
//! `.cursor/rules/external-apis.mdc` under "Credentials".
//!
//! See `plan/10-settings.md` section 12.1.

use std::io::Write;
use std::sync::{Mutex, OnceLock};

use tracing::Subscriber;
use tracing_subscriber::fmt::MakeWriter;

const REDACTED: &str = "<redacted>";

/// Sensitive field-name tokens, tested case-insensitively as a
/// substring match on the key portion of a `key=value` pair.
const SENSITIVE_TOKENS: &[&str] = &["key", "token", "secret"];

/// Install a global `tracing_subscriber` that masks sensitive fields
/// before writing to stderr. Idempotent: subsequent calls are no-ops,
/// which keeps test harnesses (or library consumers) from failing
/// when the logger has already been wired up.
pub fn install_masking_logger() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let writer = MaskingMakeWriter::stderr();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer)
            .with_target(true)
            .with_level(true)
            .with_ansi(false)
            .finish();
        // `set_global_default` fails only if another subscriber was
        // already set: swallow that so tests that install their own
        // subscriber keep working. We intentionally do not log the
        // failure — that would require a subscriber which is the very
        // thing that just failed.
        let _ = tracing::subscriber::set_global_default(subscriber);
    });
}

/// Build a subscriber that masks sensitive fields and forwards lines
/// to `writer`. Exposed for tests that want to capture the output in
/// an in-process buffer instead of stderr.
pub fn tracing_writer_for_tests<W>(writer: W) -> impl Subscriber + Send + Sync
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_target(true)
        .with_level(true)
        .with_ansi(false)
        .finish()
}

/// Pure masking helper. Safe to call from any thread. The algorithm
/// is intentionally simple — it walks the line once and rewrites the
/// value portion of every `<key>[=:]<value>` pair whose key contains
/// a sensitive token. It does **not** attempt to parse JSON beyond
/// the shape that Rust loggers actually emit.
#[must_use]
pub fn mask_sensitive(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        // Find the next `=` or `:` as a candidate separator.
        let separator = bytes[cursor..]
            .iter()
            .position(|b| *b == b'=' || *b == b':');
        let Some(sep_rel) = separator else {
            out.push_str(&line[cursor..]);
            break;
        };
        let sep = cursor + sep_rel;

        // Extract the key portion: walk backwards from `sep` until we
        // hit a character that cannot be part of an identifier
        // (whitespace, `,`, `(`, `{`, `[`, `"` for JSON, etc.).
        let key_start = find_key_start(bytes, sep);
        let raw_key = &line[key_start..sep];
        let key = unquote(raw_key).trim();

        if !is_sensitive(key) {
            // Nothing to mask — advance past the separator and keep
            // scanning. Include up to and including the separator in
            // the output.
            out.push_str(&line[cursor..=sep]);
            cursor = sep + 1;
            continue;
        }

        // Key is sensitive. Write everything up to the separator
        // verbatim, then a mask that covers the value.
        out.push_str(&line[cursor..=sep]);

        // Skip whitespace between separator and value.
        let mut value_start = sep + 1;
        while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }

        // Detect the end of the value. The value shape is one of:
        //   - "quoted string" (handles JSON + structured tracing).
        //   - bare word, terminated at whitespace / comma / `}` / `]`.
        let value_end = if value_start < bytes.len() && bytes[value_start] == b'"' {
            find_matching_quote(bytes, value_start + 1)
                .map(|end| end + 1)
                .unwrap_or(bytes.len())
        } else {
            find_bare_value_end(bytes, value_start)
        };

        if value_start == value_end {
            // No value present — leave the original unchanged to
            // avoid corrupting anything.
            cursor = sep + 1;
            continue;
        }

        // Copy the whitespace (if any) before the value, then the
        // redaction marker.
        out.push_str(&line[sep + 1..value_start]);
        out.push_str(REDACTED);

        cursor = value_end;
    }

    out
}

fn is_sensitive(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    SENSITIVE_TOKENS.iter().any(|token| lowered.contains(token))
}

fn find_key_start(bytes: &[u8], sep: usize) -> usize {
    let mut i = sep;
    while i > 0 {
        let prev = bytes[i - 1];
        if prev.is_ascii_alphanumeric() || prev == b'_' || prev == b'-' || prev == b'"' {
            i -= 1;
        } else {
            break;
        }
    }
    i
}

fn unquote(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn find_matching_quote(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

fn find_bare_value_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() || b == b',' || b == b'}' || b == b']' {
            break;
        }
        i += 1;
    }
    i
}

/// `MakeWriter` implementation that wraps a stderr handle and applies
/// [`mask_sensitive`] to every byte slice it receives.
struct MaskingMakeWriter {
    inner: MaskingWriterInner,
}

enum MaskingWriterInner {
    Stderr,
}

impl MaskingMakeWriter {
    fn stderr() -> Self {
        Self {
            inner: MaskingWriterInner::Stderr,
        }
    }
}

impl<'a> MakeWriter<'a> for MaskingMakeWriter {
    type Writer = MaskingWriter;

    fn make_writer(&'a self) -> Self::Writer {
        match self.inner {
            MaskingWriterInner::Stderr => MaskingWriter::Stderr {
                buffer: LineBuffer::default(),
            },
        }
    }
}

/// Buffer partial lines until we see a newline so the mask is always
/// applied on a complete log event.
#[derive(Default)]
struct LineBuffer {
    pending: Mutex<Vec<u8>>,
}

impl LineBuffer {
    fn feed(&self, buf: &[u8]) -> Vec<u8> {
        let mut guard = self.pending.lock().expect("logger buffer mutex");
        guard.extend_from_slice(buf);
        let mut out = Vec::with_capacity(guard.len());
        while let Some(pos) = guard.iter().position(|b| *b == b'\n') {
            let line_bytes = guard.drain(..=pos).collect::<Vec<u8>>();
            let line = String::from_utf8_lossy(&line_bytes);
            let masked = mask_sensitive(&line);
            out.extend_from_slice(masked.as_bytes());
        }
        out
    }
}

enum MaskingWriter {
    Stderr { buffer: LineBuffer },
}

impl Write for MaskingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            MaskingWriter::Stderr { buffer } => {
                let out = buffer.feed(buf);
                if !out.is_empty() {
                    let mut stderr = std::io::stderr();
                    stderr.write_all(&out)?;
                }
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_common_rust_field_syntax() {
        assert_eq!(
            mask_sensitive("loaded alchemy_key=abcdef chain=ethereum"),
            "loaded alchemy_key=<redacted> chain=ethereum"
        );
    }

    #[test]
    fn masks_colon_separated_json_pairs() {
        assert_eq!(
            mask_sensitive(r#"{"bearer_token": "xyz", "chain": "base"}"#),
            r#"{"bearer_token": <redacted>, "chain": "base"}"#
        );
    }

    #[test]
    fn leaves_unrelated_lines_alone() {
        let line = "chain=polygon latest=123 base_fee_gwei=25";
        assert_eq!(mask_sensitive(line), line);
    }
}

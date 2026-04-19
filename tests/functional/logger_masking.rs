//! Functional tests for the `mask_sensitive` helper and the
//! `install_masking_logger` end-to-end plumbing.
//!
//! See `plan/10-settings.md` section 12.1.

use std::sync::{Arc, Mutex};

use blockexplorer_tui::infra::logging::{mask_sensitive, tracing_writer_for_tests};
use pretty_assertions::assert_eq;
use tracing::{error, info};
use tracing_subscriber::fmt::MakeWriter;

#[test]
fn mask_sensitive_redacts_key_value_pairs() {
    let line = "alchemy_key=secret123 chain=ethereum";
    assert_eq!(
        mask_sensitive(line),
        "alchemy_key=<redacted> chain=ethereum"
    );
}

#[test]
fn mask_sensitive_redacts_quoted_values() {
    let line = r#"building adapter with api-token="abcdef" endpoint="https://foo""#;
    assert_eq!(
        mask_sensitive(line),
        r#"building adapter with api-token=<redacted> endpoint="https://foo""#
    );
}

#[test]
fn mask_sensitive_redacts_json_like_pairs() {
    let line = r#"{"alchemy_secret": "abc123", "chain": "ethereum"}"#;
    assert_eq!(
        mask_sensitive(line),
        r#"{"alchemy_secret": <redacted>, "chain": "ethereum"}"#
    );
}

#[test]
fn mask_sensitive_is_case_insensitive_on_the_field_name() {
    let line = "API_TOKEN=abc Bearer_Token=xyz";
    assert_eq!(mask_sensitive(line), "API_TOKEN=<redacted> Bearer_Token=<redacted>");
}

#[test]
fn mask_sensitive_keeps_lines_without_sensitive_fields_untouched() {
    let line = "loaded chain=polygon base_url=https://api.etherscan.io/v2/api";
    assert_eq!(mask_sensitive(line), line);
}

#[test]
fn mask_sensitive_handles_structured_tracing_field_syntax() {
    let line = r#"INFO blockexplorer_tui::infra: starting alchemy_key="my-secret" chain="ethereum""#;
    assert_eq!(
        mask_sensitive(line),
        r#"INFO blockexplorer_tui::infra: starting alchemy_key=<redacted> chain="ethereum""#
    );
}

#[test]
fn tracing_subscriber_scrubs_sensitive_fields_in_process() {
    let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let make_writer = InMemoryWriter {
        buffer: buffer.clone(),
    };
    let subscriber = tracing_writer_for_tests(make_writer);

    tracing::subscriber::with_default(subscriber, || {
        info!(alchemy_key = "my-secret", chain = "ethereum", "booting");
        error!(bearer_token = "tok-123", "provider request failed");
    });

    let captured = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();

    assert!(
        !captured.contains("my-secret"),
        "captured log leaked alchemy key: {captured}"
    );
    assert!(
        !captured.contains("tok-123"),
        "captured log leaked bearer token: {captured}"
    );
    assert!(
        captured.contains("<redacted>"),
        "captured log did not apply masking helper: {captured}"
    );
    assert!(
        captured.contains("chain=\"ethereum\"") || captured.contains("chain=ethereum"),
        "captured log dropped non-sensitive field: {captured}"
    );
}

#[derive(Clone)]
struct InMemoryWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl<'a> MakeWriter<'a> for InMemoryWriter {
    type Writer = InMemoryWriterHandle;

    fn make_writer(&'a self) -> Self::Writer {
        InMemoryWriterHandle {
            buffer: self.buffer.clone(),
        }
    }
}

struct InMemoryWriterHandle {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl std::io::Write for InMemoryWriterHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let masked = mask_sensitive(&String::from_utf8_lossy(buf));
        let mut guard = self.buffer.lock().unwrap();
        guard.extend_from_slice(masked.as_bytes());
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

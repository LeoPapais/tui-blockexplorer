//! Signature directory adapter.
//!
//! Implements `SignatureDirectoryPort` via the Sourcify 4byte
//! service. The Samczsun fallback listed in the original plan stays
//! deferred; Sourcify covers the canonical 4byte.directory dataset.

pub mod sourcify;

pub use sourcify::{SignatureError, SourcifySignatureDirectory};

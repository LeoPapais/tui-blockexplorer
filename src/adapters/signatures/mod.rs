//! Signature directory adapters.
//!
//! Implements the `SignatureDirectoryPort` fallback chain mandated by
//! `.cursor/rules/external-apis.mdc`:
//!
//! 1. ABI (Etherscan) — handled by `ContractSourcePort`, outside this
//!    module.
//! 2. `api.openchain.xyz` — [`HttpSignatureDirectory`].
//! 3. Samczsun signature DB mirror — [`SamczsunSignatureDirectory`].
//! 4. Raw selector / topic rendering — handled by the view.
//!
//! [`CompositeSignatureDirectory`] wires openchain + Samczsun together
//! so the application layer only sees a single port.
//!
//! See `plan/15-backlog.md` section 3.2 (probe finding that retired
//! the original Sourcify mirror).

pub mod composite;
pub mod openchain;
pub mod samczsun;

pub use composite::CompositeSignatureDirectory;
pub use openchain::{HttpSignatureDirectory, SignatureError};
pub use samczsun::SamczsunSignatureDirectory;

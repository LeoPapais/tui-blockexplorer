//! Secret-store adapters.
//!
//! MVP ships only the plaintext implementation; OS-keychain support
//! stays deferred per `plan/10-settings.md` section 12.6.

pub mod plaintext;

pub use plaintext::PlaintextSecretStore;

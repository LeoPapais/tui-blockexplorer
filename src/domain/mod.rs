//! Domain layer: pure types and invariants.
//!
//! Nothing in this module performs I/O. Value objects and entities live in
//! their own files and expose fallible constructors returning
//! [`DomainError`].
//!
//! See `plan/0-general-architecture.md` section 6 for layer rules.

pub mod errors;

pub use errors::DomainError;

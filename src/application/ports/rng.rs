//! `Rng` port — abstracts over OS-provided randomness so application
//! and adapter code can be driven deterministically in tests.
//!
//! See `plan/11-rust-scaffolding.md` §9.3 and the §8.12 item in
//! `plan/15-backlog.md`.

/// Source of random bytes.
///
/// Uses interior mutability (`&self`) so a single `Arc<dyn Rng>` can
/// be shared across tokio tasks without wrapping every call site in a
/// `Mutex`. Implementations must be `Send + Sync`.
///
/// See `plan/11-rust-scaffolding.md` §9.3.
pub trait Rng: Send + Sync {
    /// Fill `dest` with random bytes.
    ///
    /// Implementations must fill the entire slice or panic. They must
    /// not return partial results.
    fn fill_bytes(&self, dest: &mut [u8]);

    /// Draw a fresh `u64`.
    ///
    /// The default implementation pulls 8 bytes through
    /// [`Self::fill_bytes`] and interprets them as little-endian.
    fn next_u64(&self) -> u64 {
        let mut buf = [0u8; 8];
        self.fill_bytes(&mut buf);
        u64::from_le_bytes(buf)
    }
}

impl<R: Rng + ?Sized> Rng for &R {
    fn fill_bytes(&self, dest: &mut [u8]) {
        (*self).fill_bytes(dest);
    }
    fn next_u64(&self) -> u64 {
        (*self).next_u64()
    }
}

impl<R: Rng + ?Sized> Rng for std::sync::Arc<R> {
    fn fill_bytes(&self, dest: &mut [u8]) {
        (**self).fill_bytes(dest);
    }
    fn next_u64(&self) -> u64 {
        (**self).next_u64()
    }
}

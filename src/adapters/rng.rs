//! OS-backed `Rng` adapter.
//!
//! Reads from `/dev/urandom` on Unix. The adapter is not yet wired
//! into the composition root; it lands as part of §8.12 so the first
//! consumer can pick it up without a second refactor.
//!
//! See `plan/11-rust-scaffolding.md` §9.3.

use std::fs::File;
use std::io::Read;
use std::sync::Mutex;

use crate::application::ports::Rng;

/// OS-provided randomness backed by `/dev/urandom`.
///
/// The underlying file handle is opened lazily on first use and kept
/// alive for the process lifetime; `/dev/urandom` is designed for this
/// access pattern.
///
/// On non-Unix platforms this adapter currently panics on first use.
/// If Windows support becomes a requirement, pull `getrandom` from the
/// dep graph (already transitively present) and switch the impl.
pub struct OsRng {
    // Mutex<Option<File>> lets us keep `Rng::fill_bytes(&self, …)` but
    // still open the file on demand. Contention is negligible: the
    // adapter is used for occasional seed / nonce draws, not hot-path.
    file: Mutex<Option<File>>,
}

impl OsRng {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            file: Mutex::new(None),
        }
    }
}

impl Default for OsRng {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for OsRng {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OsRng").finish_non_exhaustive()
    }
}

#[cfg(unix)]
impl Rng for OsRng {
    fn fill_bytes(&self, dest: &mut [u8]) {
        let mut guard = self.file.lock().expect("OsRng lock poisoned");
        if guard.is_none() {
            let f = File::open("/dev/urandom").expect("OsRng: open /dev/urandom failed");
            *guard = Some(f);
        }
        let file = guard.as_mut().expect("OsRng: file just opened above");
        file.read_exact(dest)
            .expect("OsRng: /dev/urandom short read");
    }
}

#[cfg(not(unix))]
impl Rng for OsRng {
    fn fill_bytes(&self, _dest: &mut [u8]) {
        panic!(
            "OsRng: only /dev/urandom-backed Unix is supported; \
             see plan/11-rust-scaffolding.md §9.3 before enabling \
             non-Unix targets."
        );
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    #[test]
    fn produces_non_zero_bytes_on_unix() {
        let rng = OsRng::new();
        let mut buf = [0u8; 32];
        rng.fill_bytes(&mut buf);
        // `/dev/urandom` can technically return all-zero bytes, but the
        // probability (2^-256) is well below any test-flake threshold.
        assert!(buf.iter().any(|b| *b != 0));
    }

    #[test]
    fn next_u64_reads_eight_bytes() {
        let rng = OsRng::new();
        let a = rng.next_u64();
        let b = rng.next_u64();
        // Same 2^-64 flake-probability argument: two consecutive draws
        // collide with vanishingly small probability.
        assert_ne!(a, b);
    }
}

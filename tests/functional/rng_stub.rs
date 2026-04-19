//! Functional tests for the `Rng` port and the `SeededRng` test stub.
//!
//! See `plan/11-rust-scaffolding.md` §9.3.

use blockexplorer_tui::application::ports::Rng;
use pretty_assertions::assert_eq;

use crate::support::stubs::SeededRng;

#[test]
fn it_produces_the_same_stream_when_seeded_identically() {
    let a = SeededRng::new(42);
    let b = SeededRng::new(42);

    let mut buf_a = [0u8; 32];
    let mut buf_b = [0u8; 32];
    a.fill_bytes(&mut buf_a);
    b.fill_bytes(&mut buf_b);

    assert_eq!(buf_a, buf_b);
    assert_eq!(a.next_u64(), b.next_u64());
}

#[test]
fn it_produces_different_streams_for_different_seeds() {
    let a = SeededRng::new(1);
    let b = SeededRng::new(2);

    let mut buf_a = [0u8; 16];
    let mut buf_b = [0u8; 16];
    a.fill_bytes(&mut buf_a);
    b.fill_bytes(&mut buf_b);

    assert_ne!(buf_a, buf_b);
}

#[test]
fn fill_bytes_fills_the_whole_slice() {
    let rng = SeededRng::new(7);
    let mut buf = [0u8; 37];
    rng.fill_bytes(&mut buf);

    assert!(
        buf.iter().any(|b| *b != 0),
        "SeededRng::fill_bytes left the whole buffer zero; seed 7 is non-zero so at least one byte must be set"
    );
}

#[test]
fn next_u64_advances_state() {
    let rng = SeededRng::new(0xabcd);
    let a = rng.next_u64();
    let b = rng.next_u64();
    let c = rng.next_u64();
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
}

#[test]
#[should_panic(expected = "seed must be non-zero")]
fn zero_seed_is_rejected() {
    let _ = SeededRng::new(0);
}

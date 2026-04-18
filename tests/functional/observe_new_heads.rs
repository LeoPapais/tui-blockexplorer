//! Functional tests for `observe_new_heads`.
//!
//! See `plan/1-home.md` section 12.3.

use blockexplorer_tui::{
    application::use_cases::observe_new_heads,
    domain::{BlockNumber, Chain, DomainError, NewHead},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubNewHeadsStreamPort;

#[tokio::test]
async fn it_emits_on_push_head() {
    let port = StubNewHeadsStreamPort::new();
    let mut rx = observe_new_heads::run(&port, Chain::Ethereum)
        .await
        .expect("subscribe ok");

    let head = NewHead {
        chain: Chain::Ethereum,
        number: BlockNumber::new(21_345_679),
    };
    port.push_head(head);

    assert_eq!(rx.recv().await, Some(head));
}

#[tokio::test]
async fn it_surfaces_provider_unavailable_when_broken() {
    let port = StubNewHeadsStreamPort::new();
    port.set_broken(true);

    let err = observe_new_heads::run(&port, Chain::Ethereum)
        .await
        .expect_err("broken provider must fail fast");

    assert!(matches!(err, DomainError::ProviderUnavailable));
}

#[tokio::test]
async fn each_subscribe_call_gets_its_own_channel() {
    // plan/1-home.md §12.7: the stub fan-outs events to every active
    // subscriber. We rely on this in the BDD step that opens the Home
    // screen and then re-subscribes to check cross-chain isolation.
    let port = StubNewHeadsStreamPort::new();
    let mut a = observe_new_heads::run(&port, Chain::Ethereum)
        .await
        .unwrap();
    let mut b = observe_new_heads::run(&port, Chain::Ethereum)
        .await
        .unwrap();

    let head = NewHead {
        chain: Chain::Ethereum,
        number: BlockNumber::new(42),
    };
    port.push_head(head);

    assert_eq!(a.recv().await, Some(head));
    assert_eq!(b.recv().await, Some(head));
}

#[tokio::test]
async fn disconnect_all_drops_existing_subscribers() {
    // Models the upstream WS connection vanishing. Receivers see `None`
    // so the dispatcher can flip to Disconnected and fall back to
    // polling (plan/1-home.md §12.5).
    let port = StubNewHeadsStreamPort::new();
    let mut rx = observe_new_heads::run(&port, Chain::Ethereum)
        .await
        .unwrap();

    port.disconnect_all();

    assert_eq!(rx.recv().await, None);
}

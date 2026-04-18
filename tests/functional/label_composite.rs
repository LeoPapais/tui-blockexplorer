//! Functional tests for [`CompositeLabels`] and the static
//! [`WellKnownLabels`] table.
//!
//! See `plan/3-block-detail.md` §12.4.

use blockexplorer_tui::{
    adapters::labels::{CompositeLabels, WellKnownLabels},
    application::ports::LabelPort,
    domain::{Address, Chain, DomainError, Label, LabelSource},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::StubLabelPort;

#[tokio::test]
async fn well_known_returns_the_static_entry() {
    let mut table = WellKnownLabels::empty();
    let addr =
        Address::from_hex("0x00856730088a5c3191bd26eb482e45229555ce57").expect("hex");
    table.insert(Chain::Polygon, addr, "Polygon: Validator 1");

    let hit = table.label_for(addr, Chain::Polygon).await.unwrap();

    assert_eq!(
        hit,
        Some(Label {
            name: "Polygon: Validator 1".to_string(),
            source: LabelSource::WellKnown,
        }),
    );
}

#[tokio::test]
async fn well_known_returns_none_on_miss() {
    let table = WellKnownLabels::empty();
    let addr = Address::from_hex("0x1111111111111111111111111111111111111111").unwrap();

    let hit = table.label_for(addr, Chain::Ethereum).await.unwrap();

    assert_eq!(hit, None);
}

#[tokio::test]
async fn defaults_table_labels_the_weth_contract_on_ethereum() {
    let table = WellKnownLabels::defaults();
    let weth = Address::from_hex("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap();

    let hit = table
        .label_for(weth, Chain::Ethereum)
        .await
        .unwrap()
        .expect("WETH is in the default table");

    assert_eq!(hit.name, "WETH");
    assert_eq!(hit.source, LabelSource::WellKnown);
}

#[tokio::test]
async fn composite_prefers_the_primary_source_on_a_hit() {
    let mut primary = WellKnownLabels::empty();
    let addr = Address::from_hex("0x00856730088a5c3191bd26eb482e45229555ce57").unwrap();
    primary.insert(Chain::Polygon, addr, "Polygon: Validator 1");

    let secondary = StubLabelPort::new();
    secondary.set_label(addr, Label::etherscan("Etherscan: Validator Proxy"));

    let composite = CompositeLabels::new(primary, secondary.clone());
    let hit = composite.label_for(addr, Chain::Polygon).await.unwrap();

    assert_eq!(
        hit,
        Some(Label::well_known("Polygon: Validator 1")),
        "primary hit must win",
    );
    assert_eq!(secondary.call_count(), 0);
}

#[tokio::test]
async fn composite_falls_back_to_secondary_when_primary_misses() {
    let primary = WellKnownLabels::empty();
    let secondary = StubLabelPort::new();
    let addr = Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
    secondary.set_label(addr, Label::etherscan("USDC"));

    let composite = CompositeLabels::new(primary, secondary.clone());
    let hit = composite.label_for(addr, Chain::Ethereum).await.unwrap();

    assert_eq!(hit, Some(Label::etherscan("USDC")));
    assert_eq!(secondary.call_count(), 1);
}

#[tokio::test]
async fn composite_propagates_secondary_errors() {
    let primary = WellKnownLabels::empty();
    let secondary = StubLabelPort::new();
    secondary.fail_with(DomainError::ProviderUnavailable);

    let addr = Address::from_hex("0xdeadbeef00000000000000000000000000000000").unwrap();
    let composite = CompositeLabels::new(primary, secondary);
    let err = composite
        .label_for(addr, Chain::Ethereum)
        .await
        .expect_err("secondary failure must surface");

    assert!(matches!(err, DomainError::ProviderUnavailable));
}

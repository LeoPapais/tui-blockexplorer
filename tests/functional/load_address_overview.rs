//! Functional tests for `load_address_overview`.
//!
//! See `plan/6-address-detail.md` section 12.1.

use blockexplorer_tui::{
    application::use_cases::load_address_overview,
    domain::{Address, AddressKind, AddressOverview, Chain, DomainError, Wei},
};
use pretty_assertions::assert_eq;

use crate::support::stubs::{StubAddressReaderPort, StubEnsResolverPort};

fn sample(kind: AddressKind, hex: &str) -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: Address::from_hex(hex).unwrap(),
        balance: Wei::new(523_140_000_000_000_000_000u128),
        nonce: 1_243,
        kind,
        delegated_to: match kind {
            AddressKind::Eoa { delegated_to } => delegated_to,
            AddressKind::Contract => None,
        },
        ens_name: None,
    }
}

fn ens_without_records() -> StubEnsResolverPort {
    StubEnsResolverPort::new()
}

#[tokio::test]
async fn happy_path_for_an_eoa() {
    let reader = StubAddressReaderPort::new();
    let ens = ens_without_records();
    let ov = sample(
        AddressKind::Eoa { delegated_to: None },
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    );
    reader.insert(ov.clone());

    let got = load_address_overview::run(&reader, &ens, ov.address, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got, ov);
    assert_eq!(got.kind, AddressKind::Eoa { delegated_to: None });
    assert!(got.delegated_to.is_none());
}

#[tokio::test]
async fn happy_path_for_a_contract() {
    let reader = StubAddressReaderPort::new();
    let ens = ens_without_records();
    let ov = sample(
        AddressKind::Contract,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    );
    reader.insert(ov.clone());

    let got = load_address_overview::run(&reader, &ens, ov.address, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.kind, AddressKind::Contract);
}

#[tokio::test]
async fn happy_path_for_a_delegated_eoa() {
    let reader = StubAddressReaderPort::new();
    let ens = ens_without_records();
    let delegate = Address::from_hex("0xc0ffee000000000000000000000000000000babe").unwrap();
    let ov = sample(
        AddressKind::Eoa {
            delegated_to: Some(delegate),
        },
        "0x5abc0e99dfc7ba2c9da42f8dc91ec4128a89e919",
    );
    reader.insert(ov.clone());

    let got = load_address_overview::run(&reader, &ens, ov.address, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(
        got.kind,
        AddressKind::Eoa {
            delegated_to: Some(delegate),
        },
    );
    assert_eq!(got.delegated_to, Some(delegate));
}

#[tokio::test]
async fn missing_address_returns_not_found() {
    let reader = StubAddressReaderPort::new();
    let ens = ens_without_records();
    let addr = Address::from_hex("0x0000000000000000000000000000000000000001").unwrap();

    let err = load_address_overview::run(&reader, &ens, addr, Chain::Ethereum)
        .await
        .expect_err("missing address must error");

    assert!(matches!(err, DomainError::NotFound));
}

#[tokio::test]
async fn overlays_reverse_ens_name_when_reader_has_none() {
    // plan/6-address-detail.md §11 "Shipped": the use case fills
    // `ens_name` from the ENS port whenever the reader leaves it
    // blank (which is the production Alchemy adapter's default).
    let reader = StubAddressReaderPort::new();
    let ens = StubEnsResolverPort::new();
    let ov = sample(
        AddressKind::Eoa { delegated_to: None },
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    );
    reader.insert(ov.clone());
    ens.set_reverse(ov.address, "vitalik.eth");

    let got = load_address_overview::run(&reader, &ens, ov.address, Chain::Ethereum)
        .await
        .expect("ok");

    assert_eq!(got.ens_name.as_deref(), Some("vitalik.eth"));
}

#[tokio::test]
async fn drops_reverse_ens_error_without_masking_the_overview() {
    // plan/6 §11 "Shipped": an ENS failure must not break the
    // overview. The caller still gets the reader's payload; the
    // `ens_name` stays `None`.
    use blockexplorer_tui::{
        application::ports::EnsResolverPort,
        domain::{Chain, DomainError},
    };

    #[derive(Clone)]
    struct BrokenEns;

    impl EnsResolverPort for BrokenEns {
        async fn forward(
            &self,
            _name: &str,
            _chain: Chain,
        ) -> Result<Option<Address>, DomainError> {
            Err(DomainError::ProviderUnavailable)
        }

        async fn reverse(
            &self,
            _address: Address,
            _chain: Chain,
        ) -> Result<Option<String>, DomainError> {
            Err(DomainError::ProviderUnavailable)
        }
    }

    let reader = StubAddressReaderPort::new();
    let ens = BrokenEns;
    let ov = sample(
        AddressKind::Eoa { delegated_to: None },
        "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    );
    reader.insert(ov.clone());

    let got = load_address_overview::run(&reader, &ens, ov.address, Chain::Ethereum)
        .await
        .expect("reader result must survive the ENS failure");
    assert!(got.ens_name.is_none());
}

//! Main tab "Impl" for proxy contracts. See `plan/16-unified-address-detail.md`
//! (Contract implementation tab).

use blockexplorer_tui::adapters::ui::{AddressDetailScreen, AddressTab, address_feed};
use blockexplorer_tui::domain::{
    Address, AddressKind, AddressOverview, Chain, ContractOverview, ProxyInfo, Wei,
};
use pretty_assertions::assert_eq;

fn addr() -> Address {
    Address::from_hex("0xa0a1000000000000000000000000000000000001").unwrap()
}

fn impl_addr() -> Address {
    Address::from_hex("0xb0b1000000000000000000000000000000000002").unwrap()
}

fn contract_address_overview() -> AddressOverview {
    AddressOverview {
        chain: Chain::Ethereum,
        address: addr(),
        balance: Wei::new(0),
        nonce: 1,
        kind: AddressKind::Contract,
        delegated_to: None,
        ens_name: None,
    }
}

#[test]
fn it_shows_impl_main_tab_when_contract_overview_has_proxy() {
    let (feed, _sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, addr(), feed);
    let base = contract_address_overview();
    screen.set_overview_for_test(base.clone());
    screen.set_contract_overview_for_test(ContractOverview {
        account: base,
        proxy: Some(ProxyInfo::eip1967_slot(impl_addr())),
    });
    assert_eq!(
        screen.tabs(),
        vec![
            AddressTab::Overview,
            AddressTab::Transactions,
            AddressTab::Transfers,
            AddressTab::Portfolio,
            AddressTab::Contract,
            AddressTab::ContractImpl,
        ],
    );
}

#[test]
fn it_omits_impl_tab_when_no_proxy_metadata() {
    let (feed, _sender) = address_feed();
    let mut screen = AddressDetailScreen::loading(Chain::Ethereum, addr(), feed);
    let base = contract_address_overview();
    screen.set_overview_for_test(base.clone());
    screen.set_contract_overview_for_test(ContractOverview {
        account: base,
        proxy: None,
    });
    let tabs = screen.tabs();
    assert!(tabs.contains(&AddressTab::Contract));
    assert!(!tabs.contains(&AddressTab::ContractImpl));
}

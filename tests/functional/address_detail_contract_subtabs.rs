//! Contract sub-tab rendering tests on the unified
//! `AddressDetailScreen`. Migrated from
//! `tests/functional/contract_detail_source_highlight.rs` — the
//! original `pragma` keyword highlight check, now driven on the
//! unified screen with `initial_tab = Contract` and
//! `active_contract_sub = Source`.

use blockexplorer_tui::{
    adapters::ui::{AddressDetailScreen, AddressTab, Screen, address_feed},
    domain::{
        Address, AddressKind, AddressOverview, Chain, ContractOverview, ContractSource, SourceFile,
        Wei,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
};

fn addr() -> Address {
    Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
}

fn sample_overview() -> AddressOverview {
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

fn sample_contract() -> ContractOverview {
    ContractOverview {
        account: sample_overview(),
        proxy: None,
    }
}

fn sample_source() -> ContractSource {
    ContractSource {
        is_verified: true,
        contract_name: "Sample".into(),
        compiler_version: "v0.8.19+commit.0".into(),
        optimizer_enabled: true,
        optimizer_runs: 200,
        evm_version: "paris".into(),
        license: "MIT License (MIT)".into(),
        abi: "[]".into(),
        files: vec![SourceFile {
            path: "Sample.sol".into(),
            content: "pragma solidity ^0.8.19;\ncontract Sample {}\n".into(),
        }],
        implementation: None,
    }
}

fn build_screen() -> AddressDetailScreen {
    let (feed, _sender) = address_feed();
    let mut screen = AddressDetailScreen::with_factories_and_tab(
        Chain::Ethereum,
        addr(),
        feed,
        None,
        None,
        AddressTab::Contract,
    );
    screen.set_overview_for_test(sample_overview());
    // Publish the contract-specific overview via the test helper so
    // the Contract/Overview sub-tab has data.
    let _ = sample_contract();
    // Seed the source so the Source sub-tab renders.
    screen.set_contract_source_for_test(sample_source());
    // Hop to the Source sub-tab: one `]` press.
    screen.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
    screen
}

fn render_buffer(screen: &AddressDetailScreen, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| screen.render(frame, frame.area()))
        .expect("draw");
    terminal.backend().buffer().clone()
}

#[test]
fn pragma_on_source_subtab_keeps_the_keyword_style() {
    let screen = build_screen();
    let buf = render_buffer(&screen, 120, 24);

    let rows = buf.area.height;
    let cols = buf.area.width;
    let mut hit: Option<(u16, u16)> = None;
    for y in 0..rows {
        for x in 0..cols.saturating_sub(5) {
            let mut matches = true;
            for (i, c) in "pragma".chars().enumerate() {
                if buf[(x + i as u16, y)].symbol() != c.to_string() {
                    matches = false;
                    break;
                }
            }
            if matches {
                hit = Some((x, y));
                break;
            }
        }
        if hit.is_some() {
            break;
        }
    }
    let (x, y) = hit.expect("`pragma` must appear on the rendered Source sub-tab");
    let style = buf[(x, y)].style();
    assert_eq!(style.fg, Some(Color::Cyan));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

fn contract_screen() -> AddressDetailScreen {
    let (feed, _sender) = address_feed();
    let mut screen = AddressDetailScreen::with_factories_and_tab(
        Chain::Ethereum,
        addr(),
        feed,
        None,
        None,
        AddressTab::Contract,
    );
    screen.set_overview_for_test(sample_overview());
    let _ = sample_contract();
    screen.set_contract_source_for_test(sample_source());
    screen
}

#[test]
fn digit_keys_no_longer_select_contract_subtabs() {
    // Plan/15-backlog §8.16 released the digit keys for future use;
    // cycling sub-tabs is now `[` and `]` only.
    for digit in '1'..='6' {
        let mut screen = contract_screen();
        let before = screen.active_contract_sub();
        screen.handle_key(KeyEvent::new(KeyCode::Char(digit), KeyModifiers::NONE));
        assert_eq!(
            screen.active_contract_sub(),
            before,
            "digit `{digit}` must not change the active Contract sub-tab",
        );
    }
}

#[test]
fn contract_subtab_strip_advertises_bracket_hint() {
    // Users arrive with Contract tab active; the strip title must
    // tell them to cycle with `[` / `]`.
    let screen = contract_screen();
    let buf = render_buffer(&screen, 140, 24);
    let rows = buf.area.height;
    let cols = buf.area.width;
    let mut haystack = String::new();
    for y in 0..rows {
        for x in 0..cols {
            haystack.push_str(buf[(x, y)].symbol());
        }
        haystack.push('\n');
    }
    assert!(
        haystack.contains("[ / ]"),
        "sub-tab strip title should advertise `[ / ]` bracket hint; \
         got:\n{haystack}",
    );
    assert!(
        !haystack.contains("[1-6]"),
        "sub-tab strip must no longer advertise a digit shortcut; \
         got:\n{haystack}",
    );
}

//! Render-level check that the Contract Detail Source tab styles
//! the `pragma` keyword with the highlighter theme (plan/7 §12.5.4).
//!
//! We use `TestBackend` rather than a full BDD scenario because the
//! style check is a per-cell assertion that belongs in the
//! functional layer, not in a natural-language feature file.

use blockexplorer_tui::{
    adapters::ui::{ContractDetailScreen, Screen, contract_feed},
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

fn sample_overview() -> ContractOverview {
    ContractOverview {
        account: AddressOverview {
            chain: Chain::Ethereum,
            address: Address::from_hex("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
            balance: Wei::new(0),
            nonce: 1,
            kind: AddressKind::Contract,
            delegated_to: None,
            ens_name: None,
        },
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

/// Build a screen pre-loaded with the overview + source (skipping
/// the feed roundtrip) by invoking the public test helpers on the
/// screen. We re-use the same `ContractFeed` constructor as the
/// production code path and push state through `tick()` after
/// seeding the sender side.
fn build_screen() -> ContractDetailScreen {
    let (feed, sender) = contract_feed();
    let ov = sample_overview();
    let src = sample_source();
    // Send state directly through the sender handle, then `tick`
    // pulls it into the screen.
    sender.updates_tx.send(ov.clone()).unwrap();
    sender.source_tx.send(src).unwrap();
    let mut screen = ContractDetailScreen::loading(Chain::Ethereum, ov.account.address, feed);
    // Hop from Overview -> Source tab (one Tab press).
    screen.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    screen.tick();
    screen
}

fn render_buffer(screen: &ContractDetailScreen, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| screen.render(frame, frame.area()))
        .expect("draw");
    terminal.backend().buffer().clone()
}

#[test]
fn pragma_on_source_tab_is_rendered_with_the_keyword_style() {
    let screen = build_screen();
    let buf = render_buffer(&screen, 120, 20);

    // Locate the first cell whose glyph is 'p' on a row containing
    // "pragma". The content pane wraps inside the right column so
    // we scan the buffer for the sequence.
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
    let (x, y) = hit.expect("`pragma` must appear on the rendered Source tab");
    let style = buf[(x, y)].style();
    assert_eq!(
        style.fg,
        Some(Color::Cyan),
        "pragma should be coloured with the keyword foreground (cyan)"
    );
    assert!(
        style.add_modifier.contains(Modifier::BOLD),
        "pragma should carry the BOLD modifier for the keyword theme"
    );
}

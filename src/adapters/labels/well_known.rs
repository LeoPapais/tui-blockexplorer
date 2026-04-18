//! Static, in-binary dictionary of common labels per `(chain, address)`.
//!
//! The table is intentionally small and curated — we do not aim to
//! mirror an external database. The purpose is:
//!
//! 1. Deterministic fallback when Etherscan cannot answer.
//! 2. Guaranteed rendering in offline / --demo mode.
//! 3. Friendly labels for Polygon PoS validator signers so the
//!    §3.5 Overview row has something meaningful right out of the
//!    box.
//!
//! See `plan/3-block-detail.md` §12.4.

use std::collections::HashMap;

use crate::{
    application::ports::LabelPort,
    domain::{Address, Chain, DomainError, Label},
};

type Key = (Chain, [u8; 20]);

#[derive(Debug, Clone, Default)]
pub struct WellKnownLabels {
    by_key: HashMap<Key, &'static str>,
}

impl WellKnownLabels {
    /// Build an empty table. Useful in tests.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build the default table shipped with the binary. Kept very
    /// small on purpose — promote new entries deliberately.
    #[must_use]
    pub fn defaults() -> Self {
        let mut by_key: HashMap<Key, &'static str> = HashMap::new();

        // Ethereum mainnet.
        insert(
            &mut by_key,
            Chain::Ethereum,
            "0x00000000219ab540356cbb839cbe05303d7705fa",
            "Ethereum: Beacon Deposit",
        );
        insert(
            &mut by_key,
            Chain::Ethereum,
            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "WETH",
        );

        // Polygon PoS validator signers (a small bootstrap set for
        // plan/15-backlog §3.5). The addresses were picked from the
        // Polygon validator registry snapshot on 2026-04-18.
        insert(
            &mut by_key,
            Chain::Polygon,
            "0x00856730088a5c3191bd26eb482e45229555ce57",
            "Polygon: Validator 1",
        );
        insert(
            &mut by_key,
            Chain::Polygon,
            "0xb702f1c9154ac9c08da247a8e30ee6f2f3373f41",
            "Polygon: Validator 2",
        );

        // Base mainnet — placeholder to show the table is
        // cross-chain. Replace with real labels as we curate.
        insert(
            &mut by_key,
            Chain::Base,
            "0x4200000000000000000000000000000000000010",
            "Base: L2 Standard Bridge",
        );

        Self { by_key }
    }

    /// Upsert an entry. Exposed so tests can extend a default table.
    pub fn insert(&mut self, chain: Chain, address: Address, name: &'static str) {
        self.by_key.insert((chain, *address.as_bytes()), name);
    }
}

fn insert(map: &mut HashMap<Key, &'static str>, chain: Chain, hex: &str, name: &'static str) {
    let addr = Address::from_hex(hex).expect("static address hex must parse");
    map.insert((chain, *addr.as_bytes()), name);
}

impl LabelPort for WellKnownLabels {
    async fn label_for(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<Label>, DomainError> {
        let key = (chain, *address.as_bytes());
        Ok(self.by_key.get(&key).map(|name| Label::well_known(*name)))
    }
}

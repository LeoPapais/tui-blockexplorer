//! Values a cursor can hover over on a detail screen.
//!
//! The enum is consumed by `adapters::ui::field_cursor` (the cursor
//! widget) and by `infra::navigate::LiveNavigationFactory` (which
//! translates a value into the next screen to push). See
//! `plan/17-navigable-values.md` §2.

use crate::domain::{Address, BlockHash, BlockNumber, TxHash};

/// A single value the user can land on with the field cursor.
///
/// `Plain` is copy-only; every other variant has a canonical target
/// screen through `NavigationFactory::open`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigableValue {
    Address(Address),
    TxHash(TxHash),
    BlockNumber(BlockNumber),
    BlockHash(BlockHash),
    TokenAddress(Address),
    EnsName(String),
    Plain(String),
}

impl NavigableValue {
    /// Canonical textual form, passed to the `ClipboardPort` when the
    /// user presses `y` on top of this value.
    #[must_use]
    pub fn copy_text(&self) -> String {
        match self {
            NavigableValue::Address(a) | NavigableValue::TokenAddress(a) => a.to_hex(),
            NavigableValue::TxHash(h) => h.to_hex(),
            NavigableValue::BlockNumber(n) => n.value().to_string(),
            NavigableValue::BlockHash(h) => h.to_hex(),
            NavigableValue::EnsName(name) => name.clone(),
            NavigableValue::Plain(s) => s.clone(),
        }
    }

    /// `true` when `Enter` should ask the `NavigationFactory` to
    /// push a new screen. `Plain` values are copy-only.
    #[must_use]
    pub const fn can_navigate(&self) -> bool {
        !matches!(self, NavigableValue::Plain(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_is_copy_only() {
        let v = NavigableValue::Plain("12 gwei".to_string());
        assert_eq!(v.copy_text(), "12 gwei");
        assert!(!v.can_navigate());
    }

    #[test]
    fn address_copy_text_matches_to_hex() {
        let addr =
            Address::from_hex("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").expect("valid hex");
        let v = NavigableValue::Address(addr);
        assert_eq!(v.copy_text(), addr.to_hex());
        assert!(v.can_navigate());
    }

    #[test]
    fn block_number_copy_text_is_decimal() {
        let v = NavigableValue::BlockNumber(BlockNumber::new(21_345_678));
        assert_eq!(v.copy_text(), "21345678");
        assert!(v.can_navigate());
    }

    #[test]
    fn ens_name_copy_text_round_trips() {
        let v = NavigableValue::EnsName("vitalik.eth".to_string());
        assert_eq!(v.copy_text(), "vitalik.eth");
        assert!(v.can_navigate());
    }
}

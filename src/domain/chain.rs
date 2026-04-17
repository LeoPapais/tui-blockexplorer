//! `Chain` value object: an enum of every blockchain the app can be pointed
//! at plus associated metadata.
//!
//! See `plan/1-home.md` section 11.1.

use crate::domain::DomainError;

/// A blockchain network supported by the app. New variants must be added
/// here in a conscious step, keeping the set data-driven everywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Chain {
    Ethereum,
    EthereumSepolia,
    Base,
    Polygon,
    Optimism,
    Arbitrum,
}

impl Chain {
    /// Stable short identifier used on the CLI, in config files and in
    /// Gherkin scenarios. Lowercase, no spaces.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Chain::Ethereum => "ethereum",
            Chain::EthereumSepolia => "ethereum-sepolia",
            Chain::Base => "base",
            Chain::Polygon => "polygon",
            Chain::Optimism => "optimism",
            Chain::Arbitrum => "arbitrum",
        }
    }

    /// Human-readable display name for headers and prompts.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Chain::Ethereum => "Ethereum",
            Chain::EthereumSepolia => "Ethereum Sepolia",
            Chain::Base => "Base",
            Chain::Polygon => "Polygon",
            Chain::Optimism => "Optimism",
            Chain::Arbitrum => "Arbitrum",
        }
    }

    /// Parse a slug into a `Chain`. Unknown slugs return
    /// [`DomainError::InvalidInput`] with a helpful message.
    pub fn from_slug(slug: &str) -> Result<Self, DomainError> {
        match slug {
            "ethereum" => Ok(Chain::Ethereum),
            "ethereum-sepolia" => Ok(Chain::EthereumSepolia),
            "base" => Ok(Chain::Base),
            "polygon" => Ok(Chain::Polygon),
            "optimism" => Ok(Chain::Optimism),
            "arbitrum" => Ok(Chain::Arbitrum),
            other => Err(DomainError::InvalidInput(format!("unknown chain slug: {other}"))),
        }
    }

    /// Full list of chains enabled by default. Settings may narrow this at
    /// runtime.
    #[must_use]
    pub const fn all() -> &'static [Chain] {
        &[
            Chain::Ethereum,
            Chain::EthereumSepolia,
            Chain::Base,
            Chain::Polygon,
            Chain::Optimism,
            Chain::Arbitrum,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_roundtrip_holds_for_every_variant() {
        for chain in Chain::all() {
            let roundtripped =
                Chain::from_slug(chain.slug()).expect("known slug must parse");
            assert_eq!(*chain, roundtripped);
        }
    }

    #[test]
    fn from_slug_rejects_unknown_values() {
        let err = Chain::from_slug("solana").unwrap_err();
        assert!(matches!(err, DomainError::InvalidInput(_)));
    }
}

//! Outbound port exposing Etherscan source / ABI data for a contract.
//!
//! See `plan/4-tx-detail.md` section 12.4.1 and
//! `plan/7-contract-detail.md` section 12.4.1.

use crate::{
    application::ports::ProxyDetectionPort,
    domain::{AbiSource, Address, Chain, ContractAbi, ContractSource, DomainError, ResolvedAbi},
};

pub trait ContractSourcePort: Send + Sync {
    /// Fetch the ABI metadata for `address` on `chain`. Returns
    /// `Ok(None)` when the contract is unverified or no key is
    /// configured. Provider failures surface as `Err(DomainError::*)`.
    fn get_abi(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<ContractAbi>, DomainError>> + Send;

    /// Fetch the full verified source + compiler metadata for
    /// `address` on `chain`. Returns `Ok(None)` for unverified
    /// contracts or when the provider is not configured. Provider
    /// failures surface as `Err(DomainError::*)`.
    fn get_source(
        &self,
        address: Address,
        chain: Chain,
    ) -> impl std::future::Future<Output = Result<Option<ContractSource>, DomainError>> + Send;

    /// Resolve an ABI for `address`, following an EIP-1967-style proxy
    /// when the direct ABI is unavailable. See
    /// `plan/15-backlog.md#33-abi-decoding-for-proxy-backed-contracts-must-follow-eip-1967`.
    ///
    /// The default implementation composes [`ContractSourcePort::get_abi`]
    /// with the provided [`ProxyDetectionPort`]:
    ///
    /// 1. If the direct ABI is present, return it with
    ///    [`AbiSource::Direct`]. The detector is never consulted in
    ///    this case — the proxy ABI is the authoritative view for
    ///    anyone who cares about proxy-level entry points (e.g. the
    ///    Contract Detail Source tab).
    /// 2. If the direct ABI is missing, ask the detector. If it
    ///    reports an implementation, fetch that implementation's ABI
    ///    and return it tagged with
    ///    [`AbiSource::ProxyImplementation`].
    /// 3. Otherwise return `Ok(None)`.
    ///
    /// Callers that care about matching a specific selector / topic
    /// (for example `load_tx_overview::decode_method`) typically do
    /// not use this method because they need to try both ABIs even
    /// when the direct one is present but does not contain the
    /// target signature. They compose `get_abi` and the detector
    /// manually, with the exact cascade described in the plan.
    fn get_abi_following_proxy<P>(
        &self,
        address: Address,
        chain: Chain,
        detector: &P,
    ) -> impl std::future::Future<Output = Result<Option<ResolvedAbi>, DomainError>> + Send
    where
        P: ProxyDetectionPort + ?Sized,
    {
        async move {
            if let Some(direct) = self.get_abi(address, chain).await? {
                return Ok(Some(ResolvedAbi {
                    source_kind: AbiSource::Direct,
                    abi: direct.abi,
                }));
            }
            let Some(info) = detector.detect(address, chain).await? else {
                return Ok(None);
            };
            match self.get_abi(info.implementation, chain).await? {
                Some(impl_abi) => Ok(Some(ResolvedAbi {
                    source_kind: AbiSource::ProxyImplementation {
                        proxy: address,
                        implementation: info.implementation,
                    },
                    abi: impl_abi.abi,
                })),
                None => Ok(None),
            }
        }
    }
}

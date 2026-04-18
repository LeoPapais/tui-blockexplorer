//! Use case: load the Address Detail Overview view-model.
//!
//! The MVP slice (section 12.1) only used the reader port. The
//! April-2026 follow-up (`plan/6-address-detail.md` §11 "Shipped"
//! and `plan/15-backlog.md` §8.7) threads a reverse ENS lookup so
//! the Overview header surfaces the friendly name when the wallet
//! has one set.

use crate::{
    application::ports::{AddressReaderPort, EnsResolverPort},
    domain::{Address, AddressOverview, Chain, DomainError},
};

/// Load the overview for `address` and overlay the reverse ENS
/// name. The reader's own `ens_name` field wins when it is already
/// populated (adapters may surface cached values directly); an
/// ENS lookup failure never masks a successful overview — the
/// error is dropped and the caller sees a nameless overview.
pub async fn run<P, E>(
    reader: &P,
    ens: &E,
    address: Address,
    chain: Chain,
) -> Result<AddressOverview, DomainError>
where
    P: AddressReaderPort,
    E: EnsResolverPort,
{
    let (overview, reverse) =
        tokio::join!(reader.get(address, chain), ens.reverse(address, chain),);
    let mut overview = overview?.ok_or(DomainError::NotFound)?;
    if overview.ens_name.is_none()
        && let Ok(Some(name)) = reverse
    {
        overview.ens_name = Some(name);
    }
    Ok(overview)
}

//! Alchemy-backed [`PricesPort`] implementation.
//!
//! Calls two REST endpoints:
//!
//! - `POST tokens/by-address` (spot price for one address).
//! - `POST tokens/historical` (historical series for a window).
//!
//! The Alchemy docs express timestamps as ISO 8601 strings; the
//! adapter re-projects them into `UnixTimestamp` seconds using a
//! small RFC 3339 parser that only understands the subset Alchemy
//! emits (`YYYY-MM-DDTHH:MM:SS[.fff]Z`).
//!
//! See `plan/8-token-detail.md` section 12.4 and
//! <https://www.alchemy.com/docs/data>.

use serde::{Deserialize, Serialize};

use super::client::{PricesClient, PricesError};
use crate::{
    application::ports::PricesPort,
    domain::{
        Address, Chain, DomainError, PricePoint, PriceSeries, PriceWindow, TokenPrice,
        UnixTimestamp,
    },
};

#[derive(Debug, Clone)]
pub struct AlchemyPrices {
    client: PricesClient,
}

impl AlchemyPrices {
    #[must_use]
    pub fn new(client: PricesClient) -> Self {
        Self { client }
    }
}

// ---------------------------------------------------------------------------
// Request / response shapes
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ByAddressRequest<'a> {
    addresses: Vec<ByAddressEntry<'a>>,
}

#[derive(Serialize)]
struct ByAddressEntry<'a> {
    network: &'a str,
    address: String,
}

#[derive(Deserialize)]
struct ByAddressResponse {
    #[serde(default)]
    data: Vec<ByAddressRow>,
}

#[derive(Deserialize)]
struct ByAddressRow {
    #[serde(default)]
    prices: Vec<RawPriceEntry>,
}

#[derive(Deserialize)]
struct RawPriceEntry {
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default, rename = "lastUpdatedAt")]
    last_updated_at: Option<String>,
}

#[derive(Serialize)]
struct HistoricalRequest<'a> {
    network: &'a str,
    address: String,
    interval: &'a str,
    /// Epoch seconds emitted as a JSON number — the Alchemy
    /// Prices API accepts either an ISO 8601 string *or* a bare
    /// number. Sending the value as a string (`"1712345678"`) is
    /// rejected as "Bad Request" which previously made the Chart
    /// tab silently empty.
    #[serde(rename = "startTime")]
    start_time: u64,
    #[serde(rename = "endTime")]
    end_time: u64,
}

#[derive(Deserialize)]
struct HistoricalResponse {
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    data: Vec<HistoricalPoint>,
}

#[derive(Deserialize)]
struct HistoricalPoint {
    #[serde(default)]
    value: Option<String>,
    /// ISO 8601 or epoch seconds — we accept both.
    #[serde(default)]
    timestamp: Option<String>,
}

// ---------------------------------------------------------------------------
// Port impl
// ---------------------------------------------------------------------------

impl PricesPort for AlchemyPrices {
    async fn get_single(
        &self,
        address: Address,
        chain: Chain,
    ) -> Result<Option<TokenPrice>, DomainError> {
        let req = ByAddressRequest {
            addresses: vec![ByAddressEntry {
                network: chain.alchemy_subdomain(),
                address: address.to_hex(),
            }],
        };

        let resp: ByAddressResponse = self
            .client
            .post("tokens/by-address", &req)
            .await
            .map_err(PricesError::into_domain)?;

        let entry = resp
            .data
            .into_iter()
            .flat_map(|row| row.prices.into_iter())
            .find_map(|p| {
                let value = p.value.as_deref()?.parse::<f64>().ok()?;
                let currency = p.currency.unwrap_or_else(|| "usd".to_string());
                let as_of = p
                    .last_updated_at
                    .as_deref()
                    .and_then(parse_timestamp)
                    .unwrap_or(UnixTimestamp::from_seconds(0));
                Some(TokenPrice {
                    currency,
                    value,
                    as_of,
                })
            });
        Ok(entry)
    }

    async fn get_history(
        &self,
        address: Address,
        chain: Chain,
        window: PriceWindow,
    ) -> Result<PriceSeries, DomainError> {
        // The adapter anchors the window on the wall clock. Tests
        // inject their own fixtures via the wiremock server and the
        // epoch math is not asserted there; production callers are
        // fine with a best-effort window.
        let end = current_unix_seconds();
        let start = end.saturating_sub(window.span_seconds());

        let req = HistoricalRequest {
            network: chain.alchemy_subdomain(),
            address: address.to_hex(),
            interval: window.alchemy_interval(),
            start_time: start,
            end_time: end,
        };

        let resp: HistoricalResponse = self
            .client
            .post("tokens/historical", &req)
            .await
            .map_err(PricesError::into_domain)?;

        let mut points: Vec<PricePoint> = resp
            .data
            .into_iter()
            .filter_map(|p| {
                let value = p.value.as_deref()?.parse::<f64>().ok()?;
                let at = p
                    .timestamp
                    .as_deref()
                    .and_then(parse_timestamp)
                    .unwrap_or(UnixTimestamp::from_seconds(0));
                Some(PricePoint { at, value })
            })
            .collect();
        points.sort_by_key(|p| p.at.seconds());

        Ok(PriceSeries {
            window,
            currency: resp.currency.unwrap_or_else(|| "usd".to_string()),
            points,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Best-effort wall-clock read. The adapter only uses this to build
/// the `startTime`/`endTime` window sent to Alchemy, so the precision
/// is not critical. A clock port would be overkill here.
fn current_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Parse a timestamp emitted by the Alchemy Prices API. Accepts:
///
/// - epoch seconds as a string (`"1712345678"`),
/// - ISO 8601 / RFC 3339 with a `Z` suffix (`"2024-04-05T12:00:00Z"`,
///   `"2024-04-05T12:00:00.123Z"`).
fn parse_timestamp(s: &str) -> Option<UnixTimestamp> {
    if let Ok(secs) = s.parse::<u64>() {
        return Some(UnixTimestamp::from_seconds(secs));
    }
    let trimmed = s.trim();
    let core = trimmed.strip_suffix('Z').unwrap_or(trimmed);
    let (date, time) = core.split_once('T')?;
    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;

    let (hms, _frac) = time.split_once('.').unwrap_or((time, ""));
    let mut time_parts = hms.split(':');
    let hour: u32 = time_parts.next()?.parse().ok()?;
    let minute: u32 = time_parts.next()?.parse().ok()?;
    let second: u32 = time_parts.next().unwrap_or("0").parse().ok()?;

    let secs = ymdhms_to_epoch(year, month, day, hour, minute, second)?;
    Some(UnixTimestamp::from_seconds(secs as u64))
}

/// Convert a civil UTC date-time into Unix seconds using Howard
/// Hinnant's algorithm (see <http://howardhinnant.github.io/date_algorithms.html>).
fn ymdhms_to_epoch(y: i64, m: u32, d: u32, hh: u32, mm: u32, ss: u32) -> Option<i64> {
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + (d - 1);
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe as i64 - 719_468;
    let secs = days * 86_400 + i64::from(hh) * 3600 + i64::from(mm) * 60 + i64::from(ss);
    Some(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_epoch_seconds() {
        let ts = parse_timestamp("1712345678").expect("ok");
        assert_eq!(ts.seconds(), 1_712_345_678);
    }

    #[test]
    fn parses_iso8601_zulu() {
        let ts = parse_timestamp("2024-04-05T12:00:00Z").expect("ok");
        // 2024-04-05 12:00:00 UTC = 1712318400
        assert_eq!(ts.seconds(), 1_712_318_400);
    }

    #[test]
    fn parses_iso8601_with_fractional_seconds() {
        let ts = parse_timestamp("2024-04-05T12:00:00.123Z").expect("ok");
        assert_eq!(ts.seconds(), 1_712_318_400);
    }
}

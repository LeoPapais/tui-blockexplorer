//! Functional tests for [`CompositeSignatureDirectory`].
//!
//! Exercises the openchain → Samczsun fallback chain from
//! `plan/15-backlog.md` section 3.2 with two wiremock servers: one
//! stands in for openchain (primary), the other for Samczsun
//! (fallback). The composite is built out of [`HttpSignatureDirectory`]
//! and [`SamczsunSignatureDirectory`], both targeting their respective
//! mock URLs so provenance is carried end-to-end.

use blockexplorer_tui::{
    adapters::signatures::{
        CompositeSignatureDirectory, HttpSignatureDirectory, SamczsunSignatureDirectory,
    },
    application::{SignatureSource, ports::SignatureDirectoryPort},
};
use pretty_assertions::assert_eq;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

use crate::support::fixture_loader::load_text;

const SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];
const SELECTOR_HEX: &str = "0xa9059cbb";

struct Rig {
    composite: CompositeSignatureDirectory<HttpSignatureDirectory, SamczsunSignatureDirectory>,
    openchain_server: MockServer,
    samczsun_server: MockServer,
}

async fn build_rig() -> Rig {
    let openchain_server = MockServer::start().await;
    let samczsun_server = MockServer::start().await;
    let http = reqwest::Client::builder().build().unwrap();

    let openchain = HttpSignatureDirectory::new(
        http.clone(),
        Url::parse(&openchain_server.uri()).unwrap(),
        SignatureSource::Openchain,
    );
    let samczsun =
        SamczsunSignatureDirectory::new(http, Url::parse(&samczsun_server.uri()).unwrap());

    Rig {
        composite: CompositeSignatureDirectory::new(openchain, samczsun),
        openchain_server,
        samczsun_server,
    }
}

async fn mount_openchain_hit(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/signature-database/v1/lookup"))
        .and(query_param("function", SELECTOR_HEX))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("openchain__lookup__transfer_hit.json"),
            "application/json",
        ))
        .mount(server)
        .await;
}

async fn mount_openchain_miss(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/signature-database/v1/lookup"))
        .and(query_param("function", SELECTOR_HEX))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("openchain__lookup__transfer_miss.json"),
            "application/json",
        ))
        .mount(server)
        .await;
}

async fn mount_samczsun_hit(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/signature-database/v1/lookup"))
        .and(query_param("function", SELECTOR_HEX))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("samczsun__lookup__transfer_hit.json"),
            "application/json",
        ))
        .mount(server)
        .await;
}

async fn mount_samczsun_miss(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/signature-database/v1/lookup"))
        .and(query_param("function", SELECTOR_HEX))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            load_text("samczsun__lookup__transfer_miss.json"),
            "application/json",
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn it_prefers_openchain_when_it_returns_a_hit() {
    let rig = build_rig().await;
    mount_openchain_hit(&rig.openchain_server).await;
    // Samczsun would also answer, but the composite must never
    // query it when openchain already produced a hit.
    mount_samczsun_hit(&rig.samczsun_server).await;

    let hit = rig
        .composite
        .lookup_selector(SELECTOR)
        .await
        .expect("ok")
        .expect("signature");

    assert_eq!(hit.signature, "transfer(address,uint256)");
    assert_eq!(hit.source, SignatureSource::Openchain);

    // Guard: Samczsun must not have been called at all.
    let samczsun_requests = rig
        .samczsun_server
        .received_requests()
        .await
        .unwrap_or_default();
    assert!(
        samczsun_requests.is_empty(),
        "samczsun must not be queried when openchain hits: {samczsun_requests:?}"
    );
}

#[tokio::test]
async fn it_falls_back_to_samczsun_when_openchain_misses() {
    let rig = build_rig().await;
    mount_openchain_miss(&rig.openchain_server).await;
    mount_samczsun_hit(&rig.samczsun_server).await;

    let hit = rig
        .composite
        .lookup_selector(SELECTOR)
        .await
        .expect("ok")
        .expect("signature");

    assert_eq!(hit.signature, "transfer(address,uint256)");
    assert_eq!(hit.source, SignatureSource::Samczsun);
}

#[tokio::test]
async fn it_returns_none_when_both_miss() {
    let rig = build_rig().await;
    mount_openchain_miss(&rig.openchain_server).await;
    mount_samczsun_miss(&rig.samczsun_server).await;

    let got = rig.composite.lookup_selector(SELECTOR).await.expect("ok");
    assert!(got.is_none());
}

#[tokio::test]
async fn it_surfaces_provenance_in_the_signature_source() {
    // Two scenarios back-to-back: the first hit is tagged openchain,
    // the second tagged samczsun. Running both through the same
    // assertions nails down that provenance flows from the resolving
    // adapter rather than being hard-coded by the composite.
    let rig_primary = build_rig().await;
    mount_openchain_hit(&rig_primary.openchain_server).await;
    let hit_primary = rig_primary
        .composite
        .lookup_selector(SELECTOR)
        .await
        .expect("ok")
        .expect("signature");
    assert_eq!(hit_primary.source, SignatureSource::Openchain);

    let rig_fallback = build_rig().await;
    mount_openchain_miss(&rig_fallback.openchain_server).await;
    mount_samczsun_hit(&rig_fallback.samczsun_server).await;
    let hit_fallback = rig_fallback
        .composite
        .lookup_selector(SELECTOR)
        .await
        .expect("ok")
        .expect("signature");
    assert_eq!(hit_fallback.source, SignatureSource::Samczsun);
}

//! Functional tests for `RpcError::into_domain`.
//!
//! See `plan/13-alchemy-adapter.md` §8.1 and
//! `plan/15-backlog.md` §8.14 item 6.

use blockexplorer_tui::{adapters::rpc::RpcError, domain::DomainError};
use pretty_assertions::assert_eq;

#[test]
fn it_maps_invalid_params_to_invalid_input_when_code_is_32602() {
    let err = RpcError::Rpc {
        code: -32602,
        message: "invalid argument 0: odd-length hex".into(),
    };

    let mapped = err.into_domain();

    match mapped {
        DomainError::InvalidInput(msg) => {
            assert_eq!(msg, "invalid argument 0: odd-length hex");
        }
        other => panic!("expected InvalidInput, got {other:?}"),
    }
}

#[test]
fn it_maps_method_not_found_to_feature_unavailable_when_code_is_32601() {
    let err = RpcError::Rpc {
        code: -32601,
        message: "the method eth_feeHistory does not exist".into(),
    };

    assert!(matches!(err.into_domain(), DomainError::FeatureUnavailable));
}

#[test]
fn it_maps_alchemy_method_not_supported_to_feature_unavailable_when_code_is_32004() {
    let err = RpcError::Rpc {
        code: -32004,
        message: "Method not supported on this tier".into(),
    };

    assert!(matches!(err.into_domain(), DomainError::FeatureUnavailable));
}

#[test]
fn it_maps_rate_limit_to_provider_unavailable_when_code_is_32005() {
    let err = RpcError::Rpc {
        code: -32005,
        message: "request rate exceeded".into(),
    };

    assert!(matches!(
        err.into_domain(),
        DomainError::ProviderUnavailable
    ));
}

#[test]
fn it_maps_generic_server_errors_to_provider_unavailable_when_code_is_in_32000_range() {
    for code in [-32000_i64, -32001, -32050, -32099] {
        let err = RpcError::Rpc {
            code,
            message: "server error".into(),
        };
        let mapped = err.into_domain();
        assert!(
            matches!(mapped, DomainError::ProviderUnavailable),
            "code {code} should be ProviderUnavailable but got {mapped:?}",
        );
    }
}

#[test]
fn it_maps_unknown_rpc_codes_to_internal_so_regressions_are_obvious() {
    let err = RpcError::Rpc {
        code: -31000,
        message: "hypothetical future code".into(),
    };
    assert!(matches!(err.into_domain(), DomainError::Internal(_)));
}

#[test]
fn it_maps_rate_to_provider_unavailable() {
    assert!(matches!(
        RpcError::Rate.into_domain(),
        DomainError::ProviderUnavailable,
    ));
}

#[test]
fn it_maps_http_server_error_to_provider_unavailable() {
    assert!(matches!(
        RpcError::HttpServerError { status: 502 }.into_domain(),
        DomainError::ProviderUnavailable,
    ));
}

#[test]
fn it_maps_timeout_to_provider_unavailable() {
    assert!(matches!(
        RpcError::Timeout.into_domain(),
        DomainError::ProviderUnavailable,
    ));
}

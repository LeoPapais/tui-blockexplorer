//! Unit tests for the ABI function parser.
//!
//! See `plan/7-contract-detail.md` section 12.4.2.

use blockexplorer_tui::domain::{AbiParamType, parse_abi_functions};

#[test]
fn parses_function_entries_and_skips_events() {
    let abi = r#"[
      {"type":"function","name":"balanceOf","inputs":[{"name":"owner","type":"address"}],"outputs":[{"name":"","type":"uint256"}],"stateMutability":"view"},
      {"type":"function","name":"transfer","inputs":[{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"outputs":[],"stateMutability":"nonpayable"},
      {"type":"event","name":"Transfer","inputs":[],"anonymous":false}
    ]"#;

    let fns = parse_abi_functions(abi);
    assert_eq!(fns.len(), 2);

    let balance_of = fns.iter().find(|f| f.name == "balanceOf").unwrap();
    assert!(balance_of.is_read_only);
    assert_eq!(balance_of.signature(), "balanceOf(address)");
    assert_eq!(balance_of.inputs.len(), 1);
    assert!(matches!(balance_of.inputs[0].kind, AbiParamType::Address));
    assert!(matches!(
        balance_of.outputs[0].kind,
        AbiParamType::Uint { bits: 256 }
    ));

    let transfer = fns.iter().find(|f| f.name == "transfer").unwrap();
    assert!(!transfer.is_read_only);
}

#[test]
fn marks_unsupported_types_without_crashing() {
    let abi = r#"[
      {"type":"function","name":"swapTokens","inputs":[{"name":"path","type":"address[]"}],"outputs":[],"stateMutability":"nonpayable"}
    ]"#;
    let fns = parse_abi_functions(abi);
    assert_eq!(fns.len(), 1);
    assert!(!fns[0].is_executable());
    match &fns[0].inputs[0].kind {
        AbiParamType::Unsupported(raw) => assert_eq!(raw, "address[]"),
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

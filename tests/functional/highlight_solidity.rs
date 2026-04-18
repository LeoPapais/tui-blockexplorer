//! Functional tests for the hand-rolled Solidity highlighter.
//!
//! See `plan/7-contract-detail.md` section 12.5.4.

use blockexplorer_tui::adapters::ui::highlight::{Class, highlight_for_tests};
use pretty_assertions::assert_eq;

fn flatten(lines: Vec<Vec<(Class, String)>>) -> Vec<(Class, String)> {
    lines.into_iter().flatten().collect()
}

fn classes(pairs: &[(Class, String)]) -> Vec<Class> {
    pairs.iter().map(|(c, _)| *c).collect()
}

fn text_by_class(pairs: &[(Class, String)], class: Class) -> Vec<String> {
    pairs
        .iter()
        .filter(|(c, _)| *c == class)
        .map(|(_, t)| t.clone())
        .collect()
}

#[test]
fn it_highlights_top_level_keywords() {
    let src = "pragma solidity ^0.8.19;\ncontract Example {}\n";
    let pairs = flatten(highlight_for_tests(src));
    // `pragma` and `contract` must be classified as keywords.
    let keywords = text_by_class(&pairs, Class::Keyword);
    assert!(keywords.contains(&"pragma".to_string()));
    assert!(keywords.contains(&"contract".to_string()));
}

#[test]
fn it_highlights_numeric_literals_with_unit_suffix() {
    let src = "uint256 x = 1_000 ether;\n";
    let pairs = flatten(highlight_for_tests(src));
    // `uint256` is a Type, `ether` promotes to Number, `1_000` is a Number.
    let types = text_by_class(&pairs, Class::Type);
    assert!(types.contains(&"uint256".to_string()));
    let numbers = text_by_class(&pairs, Class::Number);
    assert!(numbers.contains(&"1_000".to_string()));
    assert!(numbers.contains(&"ether".to_string()));
}

#[test]
fn it_highlights_string_literals_end_to_end() {
    let src = r#"string constant S = "hello, world";"#;
    let pairs = flatten(highlight_for_tests(src));
    let strings = text_by_class(&pairs, Class::StringLit);
    // The highlighter must keep the surrounding quotes inside the
    // string span so the colour run matches the literal exactly.
    assert_eq!(strings, vec![r#""hello, world""#.to_string()]);
}

#[test]
fn it_swallows_line_comments_until_newline() {
    let src = "uint256 a; // trailing comment\nuint256 b;\n";
    let lines = highlight_for_tests(src);
    assert_eq!(lines.len(), 3, "trailing newline adds an empty row");
    let first = &lines[0];
    let comment = text_by_class(first, Class::Comment);
    assert_eq!(comment, vec!["// trailing comment".to_string()]);
    // The second line must not bleed into a comment.
    let second = &lines[1];
    let comments = text_by_class(second, Class::Comment);
    assert!(comments.is_empty());
}

#[test]
fn it_handles_multiline_block_comments() {
    let src = "uint256 a;\n/* start\n  middle\n  end */uint256 b;\n";
    let lines = highlight_for_tests(src);
    // Lines 1..=3 (0-indexed 1..4) are all inside the block
    // comment; the last line has a non-comment `uint256 b`.
    let classes_by_line: Vec<Vec<Class>> = lines.iter().map(|l| classes(l)).collect();
    // Line 0: no comments.
    assert!(!classes_by_line[0].contains(&Class::Comment));
    // Line 1: opens the block → first span is Comment.
    assert_eq!(classes_by_line[1][0], Class::Comment);
    // Line 2: still in block comment → the entire line is a Comment.
    assert!(classes_by_line[2].iter().all(|c| *c == Class::Comment));
    // Line 3: closes the block then starts a Type.
    assert!(classes_by_line[3].contains(&Class::Comment));
    assert!(classes_by_line[3].contains(&Class::Type));
}

#[test]
fn it_preserves_plain_identifiers_and_punctuation() {
    let src = "function foo(uint256 x) public view returns (uint256) { return x + 1; }\n";
    let pairs = flatten(highlight_for_tests(src));
    let keywords = text_by_class(&pairs, Class::Keyword);
    // Sanity: the declaration peppers multiple keywords at once.
    for kw in ["function", "public", "view", "returns", "return"] {
        assert!(
            keywords.contains(&kw.to_string()),
            "expected `{kw}` to be classified as Keyword, got keywords={keywords:?}"
        );
    }
    // `foo` and `x` are plain identifiers and must not be keywords.
    let plain = text_by_class(&pairs, Class::Plain);
    let plain_idents: Vec<String> = plain.into_iter().filter(|t| !t.trim().is_empty()).collect();
    assert!(
        plain_idents.iter().any(|s| s == "foo"),
        "expected `foo` in plain idents, got {plain_idents:?}"
    );
}

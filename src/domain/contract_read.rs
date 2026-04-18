//! Domain types for the Contract Detail "Read" tab: ABI function
//! descriptor, input values and decoded return values.
//!
//! The codec itself lives with the adapter under
//! `src/adapters/rpc/contract_reader.rs`; this module only carries
//! the provider-agnostic data shapes. See
//! `plan/7-contract-detail.md` section 12.4.2.

use std::fmt;

use crate::domain::Address;

/// Subset of Solidity ABI types we can encode / decode today.
/// Anything else surfaces as `Unsupported(raw)` so the UI can still
/// list the function while refusing to execute it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiParamType {
    Uint { bits: u16 },
    Int { bits: u16 },
    Address,
    Bool,
    String,
    Bytes,
    BytesN(u8),
    Unsupported(String),
}

impl AbiParamType {
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "address" => AbiParamType::Address,
            "bool" => AbiParamType::Bool,
            "string" => AbiParamType::String,
            "bytes" => AbiParamType::Bytes,
            s if s.starts_with("uint") => {
                let bits = s.strip_prefix("uint").unwrap_or("256");
                let bits = if bits.is_empty() {
                    256
                } else {
                    bits.parse().unwrap_or(0)
                };
                if bits == 0 || bits % 8 != 0 || bits > 256 {
                    AbiParamType::Unsupported(raw.to_string())
                } else {
                    AbiParamType::Uint { bits }
                }
            }
            s if s.starts_with("int") => {
                let bits = s.strip_prefix("int").unwrap_or("256");
                let bits = if bits.is_empty() {
                    256
                } else {
                    bits.parse().unwrap_or(0)
                };
                if bits == 0 || bits % 8 != 0 || bits > 256 {
                    AbiParamType::Unsupported(raw.to_string())
                } else {
                    AbiParamType::Int { bits }
                }
            }
            s if s.starts_with("bytes") => {
                let rest = s.strip_prefix("bytes").unwrap_or("");
                if let Ok(n) = rest.parse::<u8>()
                    && (1..=32).contains(&n)
                {
                    AbiParamType::BytesN(n)
                } else {
                    AbiParamType::Unsupported(raw.to_string())
                }
            }
            other => AbiParamType::Unsupported(other.to_string()),
        }
    }

    /// Canonical type string used inside a function signature, e.g.
    /// `uint256` or `address`. For `Unsupported` variants we echo
    /// whatever Etherscan reported.
    #[must_use]
    pub fn canonical(&self) -> String {
        match self {
            AbiParamType::Uint { bits } => format!("uint{bits}"),
            AbiParamType::Int { bits } => format!("int{bits}"),
            AbiParamType::Address => "address".to_string(),
            AbiParamType::Bool => "bool".to_string(),
            AbiParamType::String => "string".to_string(),
            AbiParamType::Bytes => "bytes".to_string(),
            AbiParamType::BytesN(n) => format!("bytes{n}"),
            AbiParamType::Unsupported(raw) => raw.clone(),
        }
    }

    #[must_use]
    pub fn is_supported(&self) -> bool {
        !matches!(self, AbiParamType::Unsupported(_))
    }
}

impl fmt::Display for AbiParamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical())
    }
}

/// Single ABI parameter — `name` stays optional because Solidity
/// does not require it for anonymous returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiParam {
    pub name: String,
    pub kind: AbiParamType,
}

/// ABI function descriptor as consumed by the Read tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiFunction {
    pub name: String,
    pub inputs: Vec<AbiParam>,
    pub outputs: Vec<AbiParam>,
    /// `true` when Solidity marks the function as view or pure.
    pub is_read_only: bool,
}

impl AbiFunction {
    /// Canonical signature used for selector hashing, e.g.
    /// `transfer(address,uint256)`.
    #[must_use]
    pub fn signature(&self) -> String {
        let params = self
            .inputs
            .iter()
            .map(|p| p.kind.canonical())
            .collect::<Vec<_>>()
            .join(",");
        format!("{}({})", self.name, params)
    }

    #[must_use]
    pub fn is_executable(&self) -> bool {
        self.is_read_only && self.inputs.iter().all(|p| p.kind.is_supported())
    }
}

/// Argument value supplied by the user. We only support the subset
/// needed to cover the common ERC-20/721 read surface:
/// no-arg calls, address args, uint args, bool args.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiValue {
    Uint(u128),
    Address(Address),
    Bool(bool),
    String(String),
}

/// Decoded return value surfaced by `ContractReaderPort::call`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedValue {
    Uint(u128),
    Int(i128),
    Address(Address),
    Bool(bool),
    String(String),
    Bytes(Vec<u8>),
}

impl fmt::Display for DecodedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodedValue::Uint(v) => write!(f, "{v}"),
            DecodedValue::Int(v) => write!(f, "{v}"),
            DecodedValue::Address(a) => write!(f, "{}", a.to_hex()),
            DecodedValue::Bool(b) => write!(f, "{b}"),
            DecodedValue::String(s) => f.write_str(s),
            DecodedValue::Bytes(b) => write!(f, "0x{}", hex::encode(b)),
        }
    }
}

/// Parse the ABI JSON (as stored on [`crate::domain::ContractSource`])
/// into a vector of [`AbiFunction`]. Events, constructors and
/// errors are skipped; unrecognised `stateMutability` values fall
/// back to not-read-only so the UI still lists them but refuses to
/// execute.
#[must_use]
pub fn parse_abi_functions(abi_json: &str) -> Vec<AbiFunction> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(abi_json) else {
        return Vec::new();
    };
    let Some(array) = value.as_array() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in array {
        if entry.get("type").and_then(serde_json::Value::as_str) != Some("function") {
            continue;
        }
        let Some(name) = entry.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let mutability = entry
            .get("stateMutability")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("nonpayable");
        let is_read_only = matches!(mutability, "view" | "pure");
        let inputs = parse_params(entry.get("inputs"));
        let outputs = parse_params(entry.get("outputs"));
        out.push(AbiFunction {
            name: name.to_string(),
            inputs,
            outputs,
            is_read_only,
        });
    }
    out
}

fn parse_params(value: Option<&serde_json::Value>) -> Vec<AbiParam> {
    let Some(array) = value.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    array
        .iter()
        .map(|p| {
            let name = p
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();
            let raw_ty = p
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            AbiParam {
                name,
                kind: AbiParamType::parse(raw_ty),
            }
        })
        .collect()
}

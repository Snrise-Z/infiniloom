//! Cross-language protocol and API contract detection
//!
//! This module identifies shared protocol definitions (currently protobuf)
//! and generates metadata for enriching embedding chunks. This enables RAG
//! systems to understand that a protobuf message like `UserRequest` is a
//! shared contract used across multiple languages/repos.
//!
//! # Supported Protocols
//!
//! - **Protobuf** (proto2 and proto3): Messages, services, enums, nested types
//!
//! # Example
//!
//! ```rust,ignore
//! use infiniloom_engine::embedding::contract_detection::{detect_contracts, contract_tags};
//!
//! let proto_content = r#"
//! syntax = "proto3";
//! package myapp.user.v1;
//!
//! service UserService {
//!     rpc GetUser(GetUserRequest) returns (GetUserResponse);
//! }
//!
//! message User {
//!     string id = 1;
//!     string name = 2;
//! }
//! "#;
//!
//! if let Some(contract) = detect_contracts(proto_content, "user.proto") {
//!     let tags = contract_tags(&contract);
//!     assert!(tags.contains(&"protobuf".to_string()));
//!     assert!(tags.contains(&"grpc".to_string()));
//!     assert_eq!(contract.services.len(), 1);
//!     assert_eq!(contract.messages.len(), 1);
//! }
//! ```

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Detected contract/protocol definition
///
/// Represents a parsed protocol definition with all its services,
/// messages, and enums. This metadata enriches embedding chunks
/// for better cross-language retrieval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractDefinition {
    /// Contract type (always "protobuf" for now)
    pub contract_type: ContractType,

    /// Package name (e.g., "myapp.user.v1")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,

    /// Service definitions found
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub services: Vec<ServiceDef>,

    /// Message definitions found
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub messages: Vec<MessageDef>,

    /// Enum definitions found
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub enums: Vec<EnumDef>,
}

/// Type of protocol/contract
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractType {
    /// Protocol Buffers (proto2 or proto3)
    Protobuf,
}

/// Service definition (gRPC service)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceDef {
    /// Service name
    pub name: String,

    /// RPC methods with signatures
    /// Format: "MethodName(RequestType) returns (ResponseType)"
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub methods: Vec<String>,
}

/// Message definition (protobuf message)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageDef {
    /// Message name
    pub name: String,

    /// Field definitions
    /// Format: "field_name: type"
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fields: Vec<String>,

    /// Nested message names (one level deep)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub nested_messages: Vec<String>,
}

/// Enum definition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumDef {
    /// Enum name
    pub name: String,

    /// Enum values
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub values: Vec<String>,
}

// Compiled regex patterns (initialized once)
static PACKAGE_RE: OnceLock<Regex> = OnceLock::new();
static MESSAGE_RE: OnceLock<Regex> = OnceLock::new();
static FIELD_RE: OnceLock<Regex> = OnceLock::new();
static SERVICE_RE: OnceLock<Regex> = OnceLock::new();
static RPC_RE: OnceLock<Regex> = OnceLock::new();
static ENUM_RE: OnceLock<Regex> = OnceLock::new();
static ENUM_VALUE_RE: OnceLock<Regex> = OnceLock::new();

/// Initialize regex patterns
fn init_patterns() {
    PACKAGE_RE.get_or_init(|| Regex::new(r"^\s*package\s+([a-zA-Z0-9_.]+)\s*;").unwrap());
    MESSAGE_RE.get_or_init(|| Regex::new(r"^\s*message\s+([a-zA-Z0-9_]+)\s*\{").unwrap());
    FIELD_RE.get_or_init(|| {
        Regex::new(
            r"^\s*(?:optional|required|repeated)?\s*([a-zA-Z0-9_.<>]+)\s+([a-zA-Z0-9_]+)\s*=\s*\d+",
        )
        .unwrap()
    });
    SERVICE_RE.get_or_init(|| Regex::new(r"^\s*service\s+([a-zA-Z0-9_]+)\s*\{").unwrap());
    RPC_RE.get_or_init(|| {
        Regex::new(
            r"^\s*rpc\s+([a-zA-Z0-9_]+)\s*\(([a-zA-Z0-9_.]+)\)\s*returns\s*\(([a-zA-Z0-9_.]+)\)",
        )
        .unwrap()
    });
    ENUM_RE.get_or_init(|| Regex::new(r"^\s*enum\s+([a-zA-Z0-9_]+)\s*\{").unwrap());
    ENUM_VALUE_RE.get_or_init(|| Regex::new(r"^\s*([a-zA-Z0-9_]+)\s*=\s*\d+").unwrap());
}

/// Detect protobuf contracts in a .proto file
///
/// Parses protobuf files and extracts package, services, messages, and enums.
/// Returns None if the file is not a .proto file.
///
/// # Arguments
///
/// * `content` - The file content to parse
/// * `file_path` - The file path (must end with .proto)
///
/// # Example
///
/// ```rust,ignore
/// let content = r#"
/// syntax = "proto3";
/// package myapp.v1;
///
/// message User {
///     string id = 1;
/// }
/// "#;
///
/// let contract = detect_contracts(content, "user.proto");
/// assert!(contract.is_some());
/// ```
pub fn detect_contracts(content: &str, file_path: &str) -> Option<ContractDefinition> {
    // Only process .proto files
    if !file_path.ends_with(".proto") {
        return None;
    }

    // Initialize regex patterns
    init_patterns();

    // Strip comments
    let content = strip_comments(content);

    // Parse contract
    let package = extract_package(&content);
    let services = extract_services(&content);
    let messages = extract_messages(&content);
    let enums = extract_enums(&content);

    // Return contract even if empty (file exists)
    Some(ContractDefinition {
        contract_type: ContractType::Protobuf,
        package,
        services,
        messages,
        enums,
    })
}

/// Generate semantic tags from contract definitions
///
/// Returns tags like `["protobuf", "grpc", "api-contract"]`.
/// Adds "grpc" tag if services are present.
///
/// # Example
///
/// ```rust,ignore
/// let contract = ContractDefinition {
///     contract_type: ContractType::Protobuf,
///     package: Some("myapp.v1".to_string()),
///     services: vec![ServiceDef {
///         name: "UserService".to_string(),
///         methods: vec![],
///     }],
///     messages: vec![],
///     enums: vec![],
/// };
///
/// let tags = contract_tags(&contract);
/// assert!(tags.contains(&"grpc".to_string()));
/// ```
pub fn contract_tags(contract: &ContractDefinition) -> Vec<String> {
    let mut tags = vec!["protobuf".to_owned(), "api-contract".to_owned()];

    // Add "grpc" if services present
    if !contract.services.is_empty() {
        tags.push("grpc".to_owned());
    }

    tags
}

/// Strip C-style and C++-style comments
fn strip_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '/' {
            if let Some(&next_ch) = chars.peek() {
                if next_ch == '/' {
                    // Line comment: skip until end of line
                    chars.next(); // consume second '/'
                    for c in chars.by_ref() {
                        if c == '\n' {
                            result.push('\n'); // preserve line breaks
                            break;
                        }
                    }
                    continue;
                } else if next_ch == '*' {
                    // Block comment: skip until */
                    chars.next(); // consume '*'
                    let mut prev = ' ';
                    for c in chars.by_ref() {
                        if prev == '*' && c == '/' {
                            break;
                        }
                        if c == '\n' {
                            result.push('\n'); // preserve line breaks
                        }
                        prev = c;
                    }
                    continue;
                }
            }
        }
        result.push(ch);
    }

    result
}

/// Extract package declaration
fn extract_package(content: &str) -> Option<String> {
    let package_re = PACKAGE_RE.get()?;
    for line in content.lines() {
        if let Some(caps) = package_re.captures(line) {
            return Some(caps[1].to_string());
        }
    }
    None
}

/// Extract service definitions
fn extract_services(content: &str) -> Vec<ServiceDef> {
    let service_re = SERVICE_RE.get().unwrap();
    let rpc_re = RPC_RE.get().unwrap();

    let mut services = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if let Some(caps) = service_re.captures(line) {
            let service_name = caps[1].to_string();
            let mut methods = Vec::new();

            // Parse methods until closing brace
            i += 1;
            let mut brace_depth = 1;
            while i < lines.len() && brace_depth > 0 {
                let method_line = lines[i];

                // Track brace depth
                brace_depth += method_line.matches('{').count();
                brace_depth -= method_line.matches('}').count();

                if let Some(rpc_caps) = rpc_re.captures(method_line) {
                    let method_name = &rpc_caps[1];
                    let request = &rpc_caps[2];
                    let response = &rpc_caps[3];
                    methods.push(format!("{}({}) returns ({})", method_name, request, response));
                }
                i += 1;
            }

            services.push(ServiceDef { name: service_name, methods });
            continue;
        }
        i += 1;
    }

    services
}

/// Extract message definitions
fn extract_messages(content: &str) -> Vec<MessageDef> {
    let message_re = MESSAGE_RE.get().unwrap();
    let field_re = FIELD_RE.get().unwrap();

    let mut messages = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if let Some(caps) = message_re.captures(line) {
            let message_name = caps[1].to_string();
            let mut fields = Vec::new();
            let mut nested_messages = Vec::new();

            // Parse fields and nested messages until closing brace
            i += 1;
            let mut brace_depth = 1;
            while i < lines.len() && brace_depth > 0 {
                let field_line = lines[i];

                // Check for nested messages before updating brace depth
                // (so we see them at depth 1 before the opening brace increments it)
                if brace_depth == 1 {
                    if let Some(nested_caps) = message_re.captures(field_line) {
                        nested_messages.push(nested_caps[1].to_string());
                    } else if let Some(field_caps) = field_re.captures(field_line) {
                        let field_type = &field_caps[1];
                        let field_name = &field_caps[2];
                        fields.push(format!("{}: {}", field_name, field_type));
                    }
                }

                // Track brace depth
                brace_depth += field_line.matches('{').count();
                brace_depth -= field_line.matches('}').count();

                i += 1;
            }

            messages.push(MessageDef { name: message_name, fields, nested_messages });
            continue;
        }
        i += 1;
    }

    messages
}

/// Extract enum definitions
fn extract_enums(content: &str) -> Vec<EnumDef> {
    let enum_re = ENUM_RE.get().unwrap();
    let enum_value_re = ENUM_VALUE_RE.get().unwrap();

    let mut enums = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if let Some(caps) = enum_re.captures(line) {
            let enum_name = caps[1].to_string();
            let mut values = Vec::new();

            // Parse values until closing brace
            i += 1;
            let mut brace_depth = 1;
            while i < lines.len() && brace_depth > 0 {
                let value_line = lines[i];

                // Track brace depth
                brace_depth += value_line.matches('{').count();
                brace_depth -= value_line.matches('}').count();

                if let Some(value_caps) = enum_value_re.captures(value_line) {
                    values.push(value_caps[1].to_string());
                }
                i += 1;
            }

            enums.push(EnumDef { name: enum_name, values });
            continue;
        }
        i += 1;
    }

    enums
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_proto_file_returns_none() {
        let content = "function foo() {}";
        let result = detect_contracts(content, "foo.js");
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_proto_file() {
        let content = r#"
syntax = "proto3";
"#;
        let result = detect_contracts(content, "empty.proto");
        assert!(result.is_some());
        let contract = result.unwrap();
        assert_eq!(contract.contract_type, ContractType::Protobuf);
        assert!(contract.package.is_none());
        assert!(contract.services.is_empty());
        assert!(contract.messages.is_empty());
        assert!(contract.enums.is_empty());
    }

    #[test]
    fn test_parse_package() {
        let content = r#"
syntax = "proto3";
package myapp.user.v1;
"#;
        let result = detect_contracts(content, "user.proto");
        assert!(result.is_some());
        let contract = result.unwrap();
        assert_eq!(contract.package, Some("myapp.user.v1".to_string()));
    }

    #[test]
    fn test_parse_message_with_fields() {
        let content = r#"
syntax = "proto3";

message User {
    string id = 1;
    string name = 2;
    int32 age = 3;
}
"#;
        let result = detect_contracts(content, "user.proto");
        assert!(result.is_some());
        let contract = result.unwrap();
        assert_eq!(contract.messages.len(), 1);

        let message = &contract.messages[0];
        assert_eq!(message.name, "User");
        assert_eq!(message.fields.len(), 3);
        assert!(message.fields.contains(&"id: string".to_string()));
        assert!(message.fields.contains(&"name: string".to_string()));
        assert!(message.fields.contains(&"age: int32".to_string()));
    }

    #[test]
    fn test_parse_service_with_rpcs() {
        let content = r#"
syntax = "proto3";

service UserService {
    rpc GetUser(GetUserRequest) returns (GetUserResponse);
    rpc CreateUser(CreateUserRequest) returns (CreateUserResponse);
}
"#;
        let result = detect_contracts(content, "user.proto");
        assert!(result.is_some());
        let contract = result.unwrap();
        assert_eq!(contract.services.len(), 1);

        let service = &contract.services[0];
        assert_eq!(service.name, "UserService");
        assert_eq!(service.methods.len(), 2);
        assert!(service
            .methods
            .contains(&"GetUser(GetUserRequest) returns (GetUserResponse)".to_string()));
        assert!(service
            .methods
            .contains(&"CreateUser(CreateUserRequest) returns (CreateUserResponse)".to_string()));
    }

    #[test]
    fn test_parse_enum() {
        let content = r#"
syntax = "proto3";

enum UserRole {
    USER_ROLE_UNSPECIFIED = 0;
    USER_ROLE_ADMIN = 1;
    USER_ROLE_MEMBER = 2;
}
"#;
        let result = detect_contracts(content, "user.proto");
        assert!(result.is_some());
        let contract = result.unwrap();
        assert_eq!(contract.enums.len(), 1);

        let enum_def = &contract.enums[0];
        assert_eq!(enum_def.name, "UserRole");
        assert_eq!(enum_def.values.len(), 3);
        assert!(enum_def
            .values
            .contains(&"USER_ROLE_UNSPECIFIED".to_string()));
        assert!(enum_def.values.contains(&"USER_ROLE_ADMIN".to_string()));
        assert!(enum_def.values.contains(&"USER_ROLE_MEMBER".to_string()));
    }

    #[test]
    fn test_parse_nested_messages() {
        let content = r#"
syntax = "proto3";

message User {
    string id = 1;

    message Address {
        string street = 1;
        string city = 2;
    }

    Address address = 2;
}
"#;
        let result = detect_contracts(content, "user.proto");
        assert!(result.is_some());
        let contract = result.unwrap();
        assert_eq!(contract.messages.len(), 1);

        let message = &contract.messages[0];
        assert_eq!(message.name, "User");
        assert_eq!(message.nested_messages.len(), 1);
        assert!(message.nested_messages.contains(&"Address".to_string()));
        // The outer message should have the Address field
        assert!(message.fields.iter().any(|f| f.contains("address")));
    }

    #[test]
    fn test_full_proto3_file() {
        let content = r#"
syntax = "proto3";

package myapp.user.v1;

// User service handles user operations
service UserService {
    rpc GetUser(GetUserRequest) returns (GetUserResponse);
    rpc CreateUser(CreateUserRequest) returns (CreateUserResponse);
}

message GetUserRequest {
    string user_id = 1;
}

message GetUserResponse {
    User user = 1;
}

message User {
    string id = 1;
    string name = 2;
    string email = 3;
    UserRole role = 4;
}

enum UserRole {
    USER_ROLE_UNSPECIFIED = 0;
    USER_ROLE_ADMIN = 1;
    USER_ROLE_MEMBER = 2;
}
"#;
        let result = detect_contracts(content, "user.proto");
        assert!(result.is_some());
        let contract = result.unwrap();

        assert_eq!(contract.package, Some("myapp.user.v1".to_string()));
        assert_eq!(contract.services.len(), 1);
        assert_eq!(contract.messages.len(), 3);
        assert_eq!(contract.enums.len(), 1);

        // Verify service
        let service = &contract.services[0];
        assert_eq!(service.name, "UserService");
        assert_eq!(service.methods.len(), 2);

        // Verify messages
        let message_names: Vec<&str> = contract.messages.iter().map(|m| m.name.as_str()).collect();
        assert!(message_names.contains(&"GetUserRequest"));
        assert!(message_names.contains(&"GetUserResponse"));
        assert!(message_names.contains(&"User"));

        // Verify enum
        let enum_def = &contract.enums[0];
        assert_eq!(enum_def.name, "UserRole");
        assert_eq!(enum_def.values.len(), 3);
    }

    #[test]
    fn test_tags_include_grpc_when_services_present() {
        let contract = ContractDefinition {
            contract_type: ContractType::Protobuf,
            package: Some("myapp.v1".to_string()),
            services: vec![ServiceDef { name: "UserService".to_string(), methods: vec![] }],
            messages: vec![],
            enums: vec![],
        };

        let tags = contract_tags(&contract);
        assert!(tags.contains(&"protobuf".to_string()));
        assert!(tags.contains(&"grpc".to_string()));
        assert!(tags.contains(&"api-contract".to_string()));
    }

    #[test]
    fn test_tags_without_services() {
        let contract = ContractDefinition {
            contract_type: ContractType::Protobuf,
            package: Some("myapp.v1".to_string()),
            services: vec![],
            messages: vec![MessageDef {
                name: "User".to_string(),
                fields: vec![],
                nested_messages: vec![],
            }],
            enums: vec![],
        };

        let tags = contract_tags(&contract);
        assert!(tags.contains(&"protobuf".to_string()));
        assert!(!tags.contains(&"grpc".to_string()));
        assert!(tags.contains(&"api-contract".to_string()));
    }

    #[test]
    fn test_comments_are_stripped() {
        let content = r#"
syntax = "proto3";

// This is a line comment
package myapp.v1;

/* This is a block comment
   spanning multiple lines */
message User {
    string id = 1; // inline comment
    string name = 2;
}
"#;
        let result = detect_contracts(content, "user.proto");
        assert!(result.is_some());
        let contract = result.unwrap();

        assert_eq!(contract.package, Some("myapp.v1".to_string()));
        assert_eq!(contract.messages.len(), 1);
        assert_eq!(contract.messages[0].name, "User");
    }
}

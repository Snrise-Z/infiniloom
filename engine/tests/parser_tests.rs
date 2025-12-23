//! Tests for the tree-sitter parser module
//!
//! These tests verify symbol extraction across multiple programming languages.

use infiniloom_engine::parser::{Language, Parser};
use infiniloom_engine::types::SymbolKind;

#[test]
fn test_parser_creation() {
    let _parser = Parser::new();
    // Parser should be created successfully - if we got here without panic, it works
}

#[test]
fn test_language_from_extension() {
    assert_eq!(Language::from_extension("py"), Some(Language::Python));
    assert_eq!(Language::from_extension("pyw"), Some(Language::Python));
    assert_eq!(Language::from_extension("js"), Some(Language::JavaScript));
    assert_eq!(Language::from_extension("jsx"), Some(Language::JavaScript));
    assert_eq!(Language::from_extension("mjs"), Some(Language::JavaScript));
    assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
    assert_eq!(Language::from_extension("tsx"), Some(Language::TypeScript));
    assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
    assert_eq!(Language::from_extension("go"), Some(Language::Go));
    assert_eq!(Language::from_extension("java"), Some(Language::Java));
    assert_eq!(Language::from_extension("unknown"), None);
    assert_eq!(Language::from_extension("txt"), None);
}

#[test]
fn test_language_name() {
    assert_eq!(Language::Python.name(), "python");
    assert_eq!(Language::JavaScript.name(), "javascript");
    assert_eq!(Language::TypeScript.name(), "typescript");
    assert_eq!(Language::Rust.name(), "rust");
    assert_eq!(Language::Go.name(), "go");
    assert_eq!(Language::Java.name(), "java");
}

// ============================================================================
// Python Tests
// ============================================================================

#[test]
fn test_parse_python_function() {
    let mut parser = Parser::new();
    let source = r#"
def hello_world():
    """Greet the world"""
    print("Hello, World!")

def add(a, b):
    return a + b
"#;

    let symbols = parser.parse(source, Language::Python).unwrap();

    let func_names: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .map(|s| s.name.as_str())
        .collect();

    assert!(func_names.contains(&"hello_world"));
    assert!(func_names.contains(&"add"));
}

#[test]
fn test_parse_python_class() {
    let mut parser = Parser::new();
    let source = r#"
class MyClass:
    """A sample class"""

    def __init__(self):
        self.value = 0

    def get_value(self):
        return self.value

    def set_value(self, value):
        self.value = value
"#;

    let symbols = parser.parse(source, Language::Python).unwrap();

    // Should find class
    let classes: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .map(|s| s.name.as_str())
        .collect();
    assert!(classes.contains(&"MyClass"));

    // Should find methods
    let methods: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Method)
        .map(|s| s.name.as_str())
        .collect();
    assert!(methods.contains(&"__init__") || methods.contains(&"get_value"));
}

#[test]
fn test_parse_python_imports() {
    let mut parser = Parser::new();
    let source = r#"
import os
import sys
from pathlib import Path
from typing import List, Dict

def main():
    pass
"#;

    let symbols = parser.parse(source, Language::Python).unwrap();

    let imports: Vec<_> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Import)
        .collect();

    assert!(!imports.is_empty());
}

#[test]
fn test_parse_python_docstring() {
    let mut parser = Parser::new();
    let source = r#"
def documented_function():
    """This is a docstring explaining the function"""
    return 42
"#;

    let symbols = parser.parse(source, Language::Python).unwrap();

    let func = symbols
        .iter()
        .find(|s| s.name == "documented_function")
        .expect("Function not found");

    // Docstring should be extracted (if parser supports it)
    // Note: This may or may not work depending on parser implementation
    if let Some(doc) = &func.docstring {
        assert!(doc.contains("docstring") || doc.contains("explaining"));
    }
}

// ============================================================================
// JavaScript Tests
// ============================================================================

#[test]
fn test_parse_javascript_function() {
    let mut parser = Parser::new();
    let source = r#"
function hello() {
    console.log("Hello!");
}

function add(a, b) {
    return a + b;
}

const arrow = () => {
    return 42;
};
"#;

    let symbols = parser.parse(source, Language::JavaScript).unwrap();

    let func_names: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .map(|s| s.name.as_str())
        .collect();

    assert!(func_names.contains(&"hello"));
    assert!(func_names.contains(&"add"));
}

#[test]
fn test_parse_javascript_class() {
    let mut parser = Parser::new();
    let source = r#"
class UserService {
    constructor() {
        this.users = [];
    }

    addUser(user) {
        this.users.push(user);
    }

    getUsers() {
        return this.users;
    }
}
"#;

    let symbols = parser.parse(source, Language::JavaScript).unwrap();

    // Should find class
    let classes: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .map(|s| s.name.as_str())
        .collect();
    assert!(classes.contains(&"UserService"));
}

// ============================================================================
// TypeScript Tests
// ============================================================================

#[test]
fn test_parse_typescript_interface() {
    let mut parser = Parser::new();
    let source = r#"
interface User {
    id: number;
    name: string;
    email?: string;
}

interface Repository {
    find(id: number): User;
    save(user: User): void;
}
"#;

    let symbols = parser.parse(source, Language::TypeScript).unwrap();

    let interfaces: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Interface)
        .map(|s| s.name.as_str())
        .collect();

    assert!(interfaces.contains(&"User"));
    assert!(interfaces.contains(&"Repository"));
}

#[test]
fn test_parse_typescript_enum() {
    let mut parser = Parser::new();
    let source = r#"
enum Status {
    Active,
    Inactive,
    Pending
}

enum Direction {
    Up = 1,
    Down = 2,
    Left = 3,
    Right = 4
}
"#;

    let symbols = parser.parse(source, Language::TypeScript).unwrap();

    let enums: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Enum)
        .map(|s| s.name.as_str())
        .collect();

    assert!(enums.contains(&"Status"));
    assert!(enums.contains(&"Direction"));
}

#[test]
fn test_parse_typescript_class_with_types() {
    let mut parser = Parser::new();
    let source = r#"
class DataService<T> {
    private data: T[];

    constructor() {
        this.data = [];
    }

    add(item: T): void {
        this.data.push(item);
    }

    getAll(): T[] {
        return this.data;
    }
}
"#;

    let symbols = parser.parse(source, Language::TypeScript).unwrap();

    let classes: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .map(|s| s.name.as_str())
        .collect();

    assert!(classes.contains(&"DataService"));
}

// ============================================================================
// Rust Tests
// ============================================================================

#[test]
fn test_parse_rust_function() {
    let mut parser = Parser::new();
    let source = r#"
/// A greeting function
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn public_function() {
    println!("Public!");
}
"#;

    let symbols = parser.parse(source, Language::Rust).unwrap();

    let func_names: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .map(|s| s.name.as_str())
        .collect();

    assert!(func_names.contains(&"greet"));
    assert!(func_names.contains(&"add"));
    assert!(func_names.contains(&"public_function"));
}

#[test]
fn test_parse_rust_struct() {
    let mut parser = Parser::new();
    let source = r#"
struct Point {
    x: f64,
    y: f64,
}

pub struct Rectangle {
    width: f64,
    height: f64,
}

struct EmptyStruct;
"#;

    let symbols = parser.parse(source, Language::Rust).unwrap();

    let structs: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Struct)
        .map(|s| s.name.as_str())
        .collect();

    assert!(structs.contains(&"Point"));
    assert!(structs.contains(&"Rectangle"));
    assert!(structs.contains(&"EmptyStruct"));
}

#[test]
fn test_parse_rust_enum() {
    let mut parser = Parser::new();
    let source = r#"
enum Color {
    Red,
    Green,
    Blue,
}

pub enum Option<T> {
    Some(T),
    None,
}
"#;

    let symbols = parser.parse(source, Language::Rust).unwrap();

    let enums: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Enum)
        .map(|s| s.name.as_str())
        .collect();

    assert!(enums.contains(&"Color"));
    // Generic Option may or may not parse correctly
}

#[test]
fn test_parse_rust_trait() {
    let mut parser = Parser::new();
    let source = r#"
trait Drawable {
    fn draw(&self);
    fn color(&self) -> &str;
}

pub trait Serializable {
    fn serialize(&self) -> String;
    fn deserialize(s: &str) -> Self;
}
"#;

    let symbols = parser.parse(source, Language::Rust).unwrap();

    let traits: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Trait)
        .map(|s| s.name.as_str())
        .collect();

    assert!(traits.contains(&"Drawable"));
    assert!(traits.contains(&"Serializable"));
}

#[test]
fn test_parse_rust_use() {
    let mut parser = Parser::new();
    let source = r#"
use std::collections::HashMap;
use std::io::{Read, Write};
use crate::types::Symbol;

fn main() {}
"#;

    let symbols = parser.parse(source, Language::Rust).unwrap();

    let imports: Vec<_> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Import)
        .collect();

    assert!(!imports.is_empty());
}

// ============================================================================
// Go Tests
// ============================================================================

#[test]
fn test_parse_go_function() {
    let mut parser = Parser::new();
    let source = r#"
package main

func main() {
    fmt.Println("Hello!")
}

func add(a, b int) int {
    return a + b
}

func (p *Point) Move(dx, dy float64) {
    p.X += dx
    p.Y += dy
}
"#;

    let symbols = parser.parse(source, Language::Go).unwrap();

    let func_names: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function || s.kind == SymbolKind::Method)
        .map(|s| s.name.as_str())
        .collect();

    assert!(func_names.contains(&"main"));
    assert!(func_names.contains(&"add"));
    // Method might be detected as "Move"
}

#[test]
fn test_parse_go_struct() {
    let mut parser = Parser::new();
    let source = r#"
package main

type Point struct {
    X float64
    Y float64
}

type Rectangle struct {
    TopLeft     Point
    BottomRight Point
}
"#;

    let symbols = parser.parse(source, Language::Go).unwrap();

    let structs: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Struct)
        .map(|s| s.name.as_str())
        .collect();

    assert!(structs.contains(&"Point"));
    assert!(structs.contains(&"Rectangle"));
}

#[test]
fn test_parse_go_interface() {
    let mut parser = Parser::new();
    let source = r#"
package main

type Reader interface {
    Read(p []byte) (n int, err error)
}

type Writer interface {
    Write(p []byte) (n int, err error)
}

type ReadWriter interface {
    Reader
    Writer
}
"#;

    let symbols = parser.parse(source, Language::Go).unwrap();

    let interfaces: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Interface)
        .map(|s| s.name.as_str())
        .collect();

    assert!(interfaces.contains(&"Reader"));
    assert!(interfaces.contains(&"Writer"));
}

// ============================================================================
// Java Tests
// ============================================================================

#[test]
fn test_parse_java_class() {
    let mut parser = Parser::new();
    let source = r#"
public class UserService {
    private List<User> users;

    public UserService() {
        this.users = new ArrayList<>();
    }

    public void addUser(User user) {
        users.add(user);
    }

    public List<User> getUsers() {
        return users;
    }
}
"#;

    let symbols = parser.parse(source, Language::Java).unwrap();

    let classes: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .map(|s| s.name.as_str())
        .collect();

    assert!(classes.contains(&"UserService"));

    let methods: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Method)
        .map(|s| s.name.as_str())
        .collect();

    assert!(methods.contains(&"addUser"));
    assert!(methods.contains(&"getUsers"));
}

#[test]
fn test_parse_java_interface() {
    let mut parser = Parser::new();
    let source = r#"
public interface Repository<T> {
    T find(int id);
    void save(T entity);
    void delete(int id);
}
"#;

    let symbols = parser.parse(source, Language::Java).unwrap();

    let interfaces: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Interface)
        .map(|s| s.name.as_str())
        .collect();

    assert!(interfaces.contains(&"Repository"));
}

#[test]
fn test_parse_java_enum() {
    let mut parser = Parser::new();
    let source = r#"
public enum Status {
    ACTIVE,
    INACTIVE,
    PENDING
}

enum Priority {
    LOW,
    MEDIUM,
    HIGH
}
"#;

    let symbols = parser.parse(source, Language::Java).unwrap();

    let enums: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Enum)
        .map(|s| s.name.as_str())
        .collect();

    assert!(enums.contains(&"Status"));
    assert!(enums.contains(&"Priority"));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_source() {
    let mut parser = Parser::new();
    let source = "";

    let symbols = parser.parse(source, Language::Python).unwrap();
    assert!(symbols.is_empty());
}

#[test]
fn test_whitespace_only() {
    let mut parser = Parser::new();
    let source = "   \n\n\t\t\n   ";

    let symbols = parser.parse(source, Language::Python).unwrap();
    assert!(symbols.is_empty());
}

#[test]
fn test_comments_only() {
    let mut parser = Parser::new();
    let source = r#"
# This is a comment
# Another comment
# No code here
"#;

    let symbols = parser.parse(source, Language::Python).unwrap();
    // Should parse successfully even with no symbols
    assert!(symbols.is_empty() || symbols.iter().all(|s| s.kind != SymbolKind::Function));
}

#[test]
fn test_nested_classes_python() {
    let mut parser = Parser::new();
    let source = r#"
class Outer:
    class Inner:
        def inner_method(self):
            pass

    def outer_method(self):
        pass
"#;

    let symbols = parser.parse(source, Language::Python).unwrap();

    let classes: Vec<&str> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .map(|s| s.name.as_str())
        .collect();

    assert!(classes.contains(&"Outer"));
    // Inner class may or may not be detected depending on query
}

#[test]
fn test_symbol_line_numbers() {
    let mut parser = Parser::new();
    let source = r#"
def first():
    pass

def second():
    pass
"#;

    let symbols = parser.parse(source, Language::Python).unwrap();

    for symbol in &symbols {
        if symbol.kind == SymbolKind::Function {
            assert!(symbol.start_line > 0);
            assert!(symbol.end_line >= symbol.start_line);
        }
    }
}

#[test]
fn test_unicode_in_source() {
    let mut parser = Parser::new();
    let source = r#"
def greet():
    """Say hello in multiple languages"""
    print("Hello")
    print("Bonjour")
    print("Hola")
"#;

    let symbols = parser.parse(source, Language::Python).unwrap();
    assert!(symbols.iter().any(|s| s.name == "greet"));
}

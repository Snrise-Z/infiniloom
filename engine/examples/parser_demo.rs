//! Example demonstrating the Tree-sitter parser capabilities
//!
//! Run with: cargo run --example parser_demo

#![allow(clippy::print_stdout)]

use infiniloom_engine::parser::{Language, Parser};

fn main() {
    println!("Infiniloom Tree-sitter Parser Demo\n");
    println!("=================================\n");

    let mut parser = Parser::new();

    // Demo 1: Python
    demo_python(&mut parser);
    println!("\n---\n");

    // Demo 2: Rust
    demo_rust(&mut parser);
    println!("\n---\n");

    // Demo 3: JavaScript
    demo_javascript(&mut parser);
    println!("\n---\n");

    // Demo 4: TypeScript
    demo_typescript(&mut parser);
    println!("\n---\n");

    // Demo 5: Go
    demo_go(&mut parser);
    println!("\n---\n");

    // Demo 6: Java
    demo_java(&mut parser);
}

fn demo_python(parser: &mut Parser) {
    println!("🐍 PYTHON PARSING");
    println!("-----------------");

    let source = r#"
import numpy as np
from typing import List, Optional

def calculate_mean(numbers: List[float]) -> float:
    """Calculate the arithmetic mean of a list of numbers.

    Args:
        numbers: A list of numeric values

    Returns:
        The mean value
    """
    return sum(numbers) / len(numbers)

class DataProcessor:
    """A class for processing numerical data."""

    def __init__(self, data: List[float]):
        self.data = data

    def process(self) -> dict:
        """Process the data and return statistics."""
        return {
            'mean': calculate_mean(self.data),
            'count': len(self.data)
        }
"#;

    match parser.parse(source, Language::Python) {
        Ok(symbols) => {
            println!("Found {} symbols:\n", symbols.len());
            for symbol in symbols {
                println!(
                    "  {} {} (lines {}-{})",
                    symbol.kind.name(),
                    symbol.name,
                    symbol.start_line,
                    symbol.end_line
                );
                if let Some(sig) = &symbol.signature {
                    println!("    Signature: {}", sig);
                }
                if let Some(doc) = &symbol.docstring {
                    println!("    Doc: {}", doc);
                }
                if let Some(parent) = &symbol.parent {
                    println!("    Parent: {}", parent);
                }
                println!();
            }
        },
        Err(e) => println!("Error: {}", e),
    }
}

fn demo_rust(parser: &mut Parser) {
    println!("🦀 RUST PARSING");
    println!("---------------");

    let source = r#"
use std::collections::HashMap;

/// A simple key-value store
pub struct Store {
    data: HashMap<String, String>,
}

impl Store {
    /// Create a new empty store
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Insert a key-value pair
    pub fn insert(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }
}

/// Calculate the factorial of a number
pub fn factorial(n: u64) -> u64 {
    match n {
        0 | 1 => 1,
        _ => n * factorial(n - 1),
    }
}

pub enum Status {
    Active,
    Inactive,
    Pending,
}
"#;

    match parser.parse(source, Language::Rust) {
        Ok(symbols) => {
            println!("Found {} symbols:\n", symbols.len());
            for symbol in symbols {
                println!(
                    "  {} {} (lines {}-{})",
                    symbol.kind.name(),
                    symbol.name,
                    symbol.start_line,
                    symbol.end_line
                );
                if let Some(doc) = &symbol.docstring {
                    println!("    Doc: {}", doc);
                }
                println!();
            }
        },
        Err(e) => println!("Error: {}", e),
    }
}

fn demo_javascript(parser: &mut Parser) {
    println!("📜 JAVASCRIPT PARSING");
    println!("---------------------");

    let source = r#"
import React from 'react';

/**
 * Calculate the sum of two numbers
 * @param {number} a - First number
 * @param {number} b - Second number
 * @returns {number} The sum
 */
function add(a, b) {
    return a + b;
}

class Counter {
    constructor(initial = 0) {
        this.count = initial;
    }

    increment() {
        this.count++;
        return this.count;
    }

    decrement() {
        this.count--;
        return this.count;
    }
}

const multiply = (x, y) => x * y;

export { add, Counter, multiply };
"#;

    match parser.parse(source, Language::JavaScript) {
        Ok(symbols) => {
            println!("Found {} symbols:\n", symbols.len());
            for symbol in symbols {
                println!(
                    "  {} {} (lines {}-{})",
                    symbol.kind.name(),
                    symbol.name,
                    symbol.start_line,
                    symbol.end_line
                );
                if let Some(sig) = &symbol.signature {
                    println!("    Signature: {}", sig.chars().take(60).collect::<String>());
                }
                println!();
            }
        },
        Err(e) => println!("Error: {}", e),
    }
}

fn demo_typescript(parser: &mut Parser) {
    println!("💎 TYPESCRIPT PARSING");
    println!("---------------------");

    let source = r#"
interface User {
    id: number;
    name: string;
    email: string;
}

enum Role {
    Admin,
    User,
    Guest
}

class UserService {
    private users: Map<number, User>;

    constructor() {
        this.users = new Map();
    }

    addUser(user: User): void {
        this.users.set(user.id, user);
    }

    getUser(id: number): User | undefined {
        return this.users.get(id);
    }
}

function validateEmail(email: string): boolean {
    const regex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return regex.test(email);
}
"#;

    match parser.parse(source, Language::TypeScript) {
        Ok(symbols) => {
            println!("Found {} symbols:\n", symbols.len());
            for symbol in symbols {
                println!(
                    "  {} {} (lines {}-{})",
                    symbol.kind.name(),
                    symbol.name,
                    symbol.start_line,
                    symbol.end_line
                );
                println!();
            }
        },
        Err(e) => println!("Error: {}", e),
    }
}

fn demo_go(parser: &mut Parser) {
    println!("🐹 GO PARSING");
    println!("-------------");

    let source = r#"
package main

import "fmt"

// User represents a user in the system
type User struct {
    ID    int
    Name  string
    Email string
}

// Calculator provides mathematical operations
type Calculator interface {
    Add(a, b int) int
    Subtract(a, b int) int
}

// Add two numbers
func Add(a, b int) int {
    return a + b
}

// Greet returns a greeting message
func (u *User) Greet() string {
    return fmt.Sprintf("Hello, %s!", u.Name)
}
"#;

    match parser.parse(source, Language::Go) {
        Ok(symbols) => {
            println!("Found {} symbols:\n", symbols.len());
            for symbol in symbols {
                println!(
                    "  {} {} (lines {}-{})",
                    symbol.kind.name(),
                    symbol.name,
                    symbol.start_line,
                    symbol.end_line
                );
                println!();
            }
        },
        Err(e) => println!("Error: {}", e),
    }
}

fn demo_java(parser: &mut Parser) {
    println!("☕ JAVA PARSING");
    println!("--------------");

    let source = r#"
package com.example.demo;

import java.util.List;
import java.util.ArrayList;

/**
 * A simple calculator class
 */
public class Calculator {
    private int result;

    public Calculator() {
        this.result = 0;
    }

    /**
     * Add two numbers
     * @param a First number
     * @param b Second number
     * @return The sum
     */
    public int add(int a, int b) {
        result = a + b;
        return result;
    }

    public int getResult() {
        return result;
    }
}

interface Operation {
    int execute(int a, int b);
}

enum MathOperation {
    ADD,
    SUBTRACT,
    MULTIPLY,
    DIVIDE
}
"#;

    match parser.parse(source, Language::Java) {
        Ok(symbols) => {
            println!("Found {} symbols:\n", symbols.len());
            for symbol in symbols {
                println!(
                    "  {} {} (lines {}-{})",
                    symbol.kind.name(),
                    symbol.name,
                    symbol.start_line,
                    symbol.end_line
                );
                if let Some(doc) = &symbol.docstring {
                    println!("    Doc: {}", doc);
                }
                println!();
            }
        },
        Err(e) => println!("Error: {}", e),
    }
}

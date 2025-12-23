//! Test Rust project for E2E testing

use serde::{Deserialize, Serialize};

/// A user in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

impl User {
    /// Create a new user
    pub fn new(id: u64, name: String, email: String) -> Self {
        Self { id, name, email }
    }

    /// Get the user's display name
    pub fn display_name(&self) -> &str {
        &self.name
    }
}

/// Calculate the sum of two numbers
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Process a list of items
pub fn process_items<T: Clone>(items: &[T]) -> Vec<T> {
    items.to_vec()
}

fn main() {
    let user = User::new(1, "Alice".to_string(), "alice@example.com".to_string());
    println!("Hello, {}!", user.display_name());
    println!("2 + 3 = {}", add(2, 3));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_user_creation() {
        let user = User::new(1, "Test".to_string(), "test@test.com".to_string());
        assert_eq!(user.id, 1);
    }
}

//! Rust Syntax Highlighting Test
//! This module demonstrates various Rust syntax constructs.

use std::collections::HashMap;
use std::fmt::{self, Display};
use std::io::{self, Read, Write};

/// A person with a name and age.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub name: String,
    pub age: u32,
    email: Option<String>,
}

impl Person {
    /// Creates a new person.
    pub fn new(name: impl Into<String>, age: u32) -> Self {
        Self {
            name: name.into(),
            age,
            email: None,
        }
    }

    /// Sets the email address.
    pub fn with_email(mut self, email: &str) -> Self {
        self.email = Some(email.to_string());
        self
    }

    /// Returns true if the person is an adult.
    pub fn is_adult(&self) -> bool {
        self.age >= 18
    }
}

impl Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.age)
    }
}

/// Status of an operation.
#[derive(Debug)]
pub enum Status {
    Pending,
    InProgress { progress: f32 },
    Completed(String),
    Failed { error: String, code: i32 },
}

/// A generic container.
pub struct Container<T> {
    items: Vec<T>,
}

impl<T: Clone> Container<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(&mut self, item: T) {
        self.items.push(item);
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }
}

impl<T: Clone> Default for Container<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Demonstrates various Rust features.
pub fn demo() -> io::Result<()> {
    // Variables and mutability
    let x = 42;
    let mut y = 10;
    y += x;

    // Different number literals
    let decimal = 1_000_000;
    let hex = 0xDEAD_BEEF;
    let octal = 0o755;
    let binary = 0b1010_1010;
    let float = 3.14159_f64;
    let scientific = 1.0e-10;

    // Strings
    let s1 = "Hello, World!";
    let s2 = String::from("Rust is awesome");
    let s3 = format!("x = {}, y = {}", x, y);
    let raw = r#"Raw string with "quotes" inside"#;
    let byte_string = b"byte string";

    // Characters
    let c = 'a';
    let emoji = '🦀';
    let escape = '\n';

    // Collections
    let array: [i32; 5] = [1, 2, 3, 4, 5];
    let tuple = (1, "hello", 3.14);
    let mut map = HashMap::new();
    map.insert("key", "value");

    // Control flow
    if x > 0 {
        println!("Positive");
    } else if x < 0 {
        println!("Negative");
    } else {
        println!("Zero");
    }

    // Pattern matching
    let status = Status::InProgress { progress: 0.5 };
    match status {
        Status::Pending => println!("Pending"),
        Status::InProgress { progress } if progress > 0.5 => println!("More than half done"),
        Status::InProgress { progress } => println!("Progress: {:.1}%", progress * 100.0),
        Status::Completed(msg) => println!("Done: {}", msg),
        Status::Failed { error, code } => eprintln!("Error {}: {}", code, error),
    }

    // Loops
    for i in 0..10 {
        println!("{}", i);
    }

    while y > 0 {
        y -= 1;
    }

    let result = loop {
        if x > 0 {
            break x * 2;
        }
    };

    // Closures
    let add = |a: i32, b: i32| a + b;
    let multiply = |a, b| a * b;
    let captured = |x| x + y;

    // Iterators
    let sum: i32 = (1..=100)
        .filter(|n| n % 2 == 0)
        .map(|n| n * n)
        .sum();

    // Error handling
    let file = std::fs::File::open("test.txt")?;
    let contents = std::fs::read_to_string("test.txt").unwrap_or_default();

    // Macros
    println!("Hello, {}!", s1);
    vec![1, 2, 3];
    assert_eq!(2 + 2, 4);
    todo!("implement this");

    Ok(())
}

/// Async function example.
pub async fn fetch_data(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Simulated async operation
    Ok(format!("Data from {}", url))
}

/// Trait definition.
pub trait Greet {
    fn greet(&self) -> String;
    
    fn greet_loudly(&self) -> String {
        self.greet().to_uppercase()
    }
}

impl Greet for Person {
    fn greet(&self) -> String {
        format!("Hello, I'm {}!", self.name)
    }
}

// Conditional compilation
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_person() {
        let p = Person::new("Alice", 30);
        assert!(p.is_adult());
        assert_eq!(p.name, "Alice");
    }

    #[test]
    #[should_panic]
    fn test_panic() {
        panic!("This test should panic");
    }
}

// Unsafe code
pub unsafe fn dangerous() {
    let raw_ptr: *const i32 = &42;
    println!("Value: {}", *raw_ptr);
}

// FFI example
#[no_mangle]
pub extern "C" fn rust_function(x: i32) -> i32 {
    x * 2
}

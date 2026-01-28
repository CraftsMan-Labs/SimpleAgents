use serde::{Deserialize, Serialize};
use simple_agents_macros::PartialType;

#[derive(Debug, Clone, PartialEq, PartialType, Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,
    age: u32,
}

#[derive(Debug, Clone, PartialEq, PartialType, Serialize, Deserialize)]
struct Resume {
    name: String,
    email: String,
    #[partial(default)]
    skills: Vec<String>,
}

#[test]
fn test_partial_type_generated() {
    // Verify the partial type exists and can be constructed
    let partial = PartialUser::default();
    assert_eq!(partial.id, None);
    assert_eq!(partial.name, None);
    assert_eq!(partial.email, None);
    assert_eq!(partial.age, None);
}

#[test]
fn test_partial_type_construction() {
    let partial = PartialUser {
        id: Some(1),
        name: Some("Alice".to_string()),
        email: Some("alice@example.com".to_string()),
        age: Some(30),
    };

    assert_eq!(partial.id, Some(1));
    assert_eq!(partial.name, Some("Alice".to_string()));
    assert_eq!(partial.email, Some("alice@example.com".to_string()));
    assert_eq!(partial.age, Some(30));
}

#[test]
fn test_from_partial_success() {
    let partial = PartialUser {
        id: Some(1),
        name: Some("Alice".to_string()),
        email: Some("alice@example.com".to_string()),
        age: Some(30),
    };

    let user = User::from_partial(partial).unwrap();

    assert_eq!(user.id, 1);
    assert_eq!(user.name, "Alice");
    assert_eq!(user.email, "alice@example.com");
    assert_eq!(user.age, 30);
}

#[test]
fn test_from_partial_missing_field() {
    let partial = PartialUser {
        id: Some(1),
        name: Some("Alice".to_string()),
        email: None, // Missing required field
        age: Some(30),
    };

    let result = User::from_partial(partial);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("email"));
}

#[test]
fn test_merge_empty_partials() {
    let mut partial1 = PartialUser::default();
    let partial2 = PartialUser::default();

    partial1.merge(partial2);

    assert_eq!(partial1.id, None);
    assert_eq!(partial1.name, None);
    assert_eq!(partial1.email, None);
    assert_eq!(partial1.age, None);
}

#[test]
fn test_merge_partial_values() {
    let mut partial1 = PartialUser {
        id: Some(1),
        name: Some("Alice".to_string()),
        email: None,
        age: None,
    };

    let partial2 = PartialUser {
        id: None,
        name: None,
        email: Some("alice@example.com".to_string()),
        age: Some(30),
    };

    partial1.merge(partial2);

    assert_eq!(partial1.id, Some(1)); // Kept from partial1
    assert_eq!(partial1.name, Some("Alice".to_string())); // Kept from partial1
    assert_eq!(partial1.email, Some("alice@example.com".to_string())); // Added from partial2
    assert_eq!(partial1.age, Some(30)); // Added from partial2
}

#[test]
fn test_merge_overwrites_existing() {
    let mut partial1 = PartialUser {
        id: Some(1),
        name: Some("Alice".to_string()),
        email: Some("old@example.com".to_string()),
        age: Some(25),
    };

    let partial2 = PartialUser {
        id: None,
        name: Some("Alice Smith".to_string()),
        email: Some("new@example.com".to_string()),
        age: None,
    };

    partial1.merge(partial2);

    assert_eq!(partial1.id, Some(1)); // Unchanged
    assert_eq!(partial1.name, Some("Alice Smith".to_string())); // Overwritten
    assert_eq!(partial1.email, Some("new@example.com".to_string())); // Overwritten
    assert_eq!(partial1.age, Some(25)); // Unchanged
}

#[test]
fn test_streaming_simulation() {
    // Simulate streaming JSON: chunks arrive progressively
    let mut partial = PartialUser::default();

    // Chunk 1: {"id": 1, "name": "Alice"}
    partial.merge(PartialUser {
        id: Some(1),
        name: Some("Alice".to_string()),
        email: None,
        age: None,
    });

    assert_eq!(partial.id, Some(1));
    assert_eq!(partial.name, Some("Alice".to_string()));
    assert!(User::from_partial(partial.clone()).is_err()); // Not complete yet

    // Chunk 2: {"email": "alice@example.com"}
    partial.merge(PartialUser {
        id: None,
        name: None,
        email: Some("alice@example.com".to_string()),
        age: None,
    });

    assert_eq!(partial.email, Some("alice@example.com".to_string()));
    assert!(User::from_partial(partial.clone()).is_err()); // Still missing age

    // Chunk 3: {"age": 30}
    partial.merge(PartialUser {
        id: None,
        name: None,
        email: None,
        age: Some(30),
    });

    assert_eq!(partial.age, Some(30));

    // Now we can convert to complete type
    let user = User::from_partial(partial).unwrap();
    assert_eq!(user.id, 1);
    assert_eq!(user.name, "Alice");
    assert_eq!(user.email, "alice@example.com");
    assert_eq!(user.age, 30);
}

#[test]
fn test_default_attribute() {
    let partial = PartialResume {
        name: Some("Alice".to_string()),
        email: Some("alice@example.com".to_string()),
        skills: None, // Missing, but has #[partial(default)]
    };

    let resume = Resume::from_partial(partial).unwrap();

    assert_eq!(resume.name, "Alice");
    assert_eq!(resume.email, "alice@example.com");
    assert_eq!(resume.skills, Vec::<String>::new()); // Uses default (empty vec)
}

#[test]
fn test_default_attribute_with_value() {
    let partial = PartialResume {
        name: Some("Bob".to_string()),
        email: Some("bob@example.com".to_string()),
        skills: Some(vec!["Rust".to_string(), "Python".to_string()]),
    };

    let resume = Resume::from_partial(partial).unwrap();

    assert_eq!(resume.name, "Bob");
    assert_eq!(resume.email, "bob@example.com");
    assert_eq!(resume.skills, vec!["Rust", "Python"]);
}

#[test]
fn test_serde_serialization() {
    let partial = PartialUser {
        id: Some(1),
        name: Some("Alice".to_string()),
        email: None,
        age: Some(30),
    };

    let json = serde_json::to_string(&partial).unwrap();
    assert!(json.contains("\"id\":1"));
    assert!(json.contains("\"name\":\"Alice\""));
    assert!(json.contains("\"age\":30"));

    let deserialized: PartialUser = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, Some(1));
    assert_eq!(deserialized.name, Some("Alice".to_string()));
    assert_eq!(deserialized.email, None);
    assert_eq!(deserialized.age, Some(30));
}

#[test]
fn test_serde_deserialization_with_nulls() {
    let json = r#"{"id": 1, "name": "Alice", "email": null, "age": 30}"#;
    let partial: PartialUser = serde_json::from_str(json).unwrap();

    assert_eq!(partial.id, Some(1));
    assert_eq!(partial.name, Some("Alice".to_string()));
    assert_eq!(partial.email, None);
    assert_eq!(partial.age, Some(30));
}

#[test]
fn test_serde_deserialization_missing_fields() {
    let json = r#"{"id": 1, "name": "Alice"}"#;
    let partial: PartialUser = serde_json::from_str(json).unwrap();

    assert_eq!(partial.id, Some(1));
    assert_eq!(partial.name, Some("Alice".to_string()));
    assert_eq!(partial.email, None);
    assert_eq!(partial.age, None);
}

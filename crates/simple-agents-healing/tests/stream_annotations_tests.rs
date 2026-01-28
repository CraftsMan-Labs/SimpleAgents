use serde_json::json;
use simple_agents_healing::schema::{Field, ObjectSchema, Schema, StreamAnnotation};

#[test]
fn test_stream_annotation_default() {
    let field = Field::required("name", Schema::String);
    assert_eq!(field.stream_annotation, StreamAnnotation::Normal);
}

#[test]
fn test_stream_annotation_not_null() {
    let field =
        Field::required("id", Schema::Int).with_stream_annotation(StreamAnnotation::NotNull);

    assert_eq!(field.stream_annotation, StreamAnnotation::NotNull);
    assert_eq!(field.name, "id");
    assert!(field.required);
}

#[test]
fn test_stream_annotation_done() {
    let field =
        Field::required("status", Schema::String).with_stream_annotation(StreamAnnotation::Done);

    assert_eq!(field.stream_annotation, StreamAnnotation::Done);
    assert_eq!(field.name, "status");
    assert!(field.required);
}

#[test]
fn test_multiple_fields_with_different_annotations() {
    let schema = Schema::Object(ObjectSchema::new(vec![
        Field::required("id", Schema::Int).with_stream_annotation(StreamAnnotation::NotNull),
        Field::required("name", Schema::String).with_stream_annotation(StreamAnnotation::Normal),
        Field::required("status", Schema::String).with_stream_annotation(StreamAnnotation::Done),
    ]));

    if let Schema::Object(obj_schema) = schema {
        assert_eq!(obj_schema.fields.len(), 3);

        // Check ID field
        let id_field = obj_schema.get_field("id").unwrap();
        assert_eq!(id_field.stream_annotation, StreamAnnotation::NotNull);

        // Check name field
        let name_field = obj_schema.get_field("name").unwrap();
        assert_eq!(name_field.stream_annotation, StreamAnnotation::Normal);

        // Check status field
        let status_field = obj_schema.get_field("status").unwrap();
        assert_eq!(status_field.stream_annotation, StreamAnnotation::Done);
    } else {
        panic!("Expected Object schema");
    }
}

#[test]
fn test_stream_annotation_with_optional_field() {
    let field =
        Field::optional("email", Schema::String).with_stream_annotation(StreamAnnotation::NotNull);

    assert_eq!(field.stream_annotation, StreamAnnotation::NotNull);
    assert_eq!(field.name, "email");
    assert!(!field.required);
}

#[test]
fn test_stream_annotation_with_default_value() {
    let field = Field::optional("count", Schema::Int)
        .with_default(json!(0))
        .with_stream_annotation(StreamAnnotation::NotNull);

    assert_eq!(field.stream_annotation, StreamAnnotation::NotNull);
    assert_eq!(field.default, Some(json!(0)));
}

#[test]
fn test_stream_annotation_with_alias() {
    let field = Field::required("user_id", Schema::Int)
        .with_alias("userId")
        .with_stream_annotation(StreamAnnotation::NotNull);

    assert_eq!(field.stream_annotation, StreamAnnotation::NotNull);
    assert_eq!(field.aliases, vec!["userId"]);
}

#[test]
fn test_stream_annotation_with_description() {
    let field = Field::required("id", Schema::Int)
        .with_description("Unique identifier")
        .with_stream_annotation(StreamAnnotation::NotNull);

    assert_eq!(field.stream_annotation, StreamAnnotation::NotNull);
    assert_eq!(field.description, Some("Unique identifier".to_string()));
}

#[test]
fn test_stream_annotation_serialization() {
    let field =
        Field::required("id", Schema::Int).with_stream_annotation(StreamAnnotation::NotNull);

    // Serialize to JSON
    let json = serde_json::to_string(&field).unwrap();
    assert!(json.contains("NotNull"));

    // Deserialize back
    let deserialized: Field = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.stream_annotation, StreamAnnotation::NotNull);
}

#[test]
fn test_stream_annotation_normal_is_default() {
    // When deserializing without stream_annotation field, should default to Normal
    let json = r#"{
        "name": "test",
        "schema": "String",
        "required": true,
        "aliases": [],
        "default": null,
        "description": null
    }"#;

    let field: Field = serde_json::from_str(json).unwrap();
    assert_eq!(field.stream_annotation, StreamAnnotation::Normal);
}

#[test]
fn test_stream_annotation_builder_chain() {
    let field = Field::required("user", Schema::Object(ObjectSchema::new(vec![])))
        .with_alias("userProfile")
        .with_description("User profile object")
        .with_stream_annotation(StreamAnnotation::Done);

    assert_eq!(field.name, "user");
    assert_eq!(field.aliases, vec!["userProfile"]);
    assert_eq!(field.description, Some("User profile object".to_string()));
    assert_eq!(field.stream_annotation, StreamAnnotation::Done);
}

#[test]
fn test_nested_object_with_stream_annotations() {
    let inner_schema = Schema::Object(ObjectSchema::new(vec![
        Field::required("id", Schema::Int).with_stream_annotation(StreamAnnotation::NotNull),
        Field::required("name", Schema::String),
    ]));

    let outer_schema = Schema::Object(ObjectSchema::new(vec![Field::required(
        "user",
        inner_schema,
    )
    .with_stream_annotation(StreamAnnotation::Done)]));

    if let Schema::Object(obj_schema) = outer_schema {
        let user_field = obj_schema.get_field("user").unwrap();
        assert_eq!(user_field.stream_annotation, StreamAnnotation::Done);

        if let Schema::Object(inner_obj) = &user_field.schema {
            let id_field = inner_obj.get_field("id").unwrap();
            assert_eq!(id_field.stream_annotation, StreamAnnotation::NotNull);
        } else {
            panic!("Expected Object schema for user field");
        }
    } else {
        panic!("Expected Object schema");
    }
}

#[test]
fn test_stream_annotation_copy_trait() {
    let annotation1 = StreamAnnotation::NotNull;
    let annotation2 = annotation1; // Copy, not move

    assert_eq!(annotation1, StreamAnnotation::NotNull);
    assert_eq!(annotation2, StreamAnnotation::NotNull);
}

#[test]
fn test_stream_annotation_eq_trait() {
    assert_eq!(StreamAnnotation::Normal, StreamAnnotation::Normal);
    assert_eq!(StreamAnnotation::NotNull, StreamAnnotation::NotNull);
    assert_eq!(StreamAnnotation::Done, StreamAnnotation::Done);

    assert_ne!(StreamAnnotation::Normal, StreamAnnotation::NotNull);
    assert_ne!(StreamAnnotation::Normal, StreamAnnotation::Done);
    assert_ne!(StreamAnnotation::NotNull, StreamAnnotation::Done);
}

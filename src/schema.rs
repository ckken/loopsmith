use serde_json::{Value, json};

fn object_schema(properties: Value, required: Vec<&str>) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

pub fn review_schema() -> Value {
    let finding = object_schema(
        json!({
            "artifact": {"type": "string"},
            "issue_type": {"type": "string"},
            "severity": {"type": "string"},
            "description": {"type": "string"},
            "suggested_fix_direction": {"type": "string"}
        }),
        vec![
            "artifact",
            "issue_type",
            "severity",
            "description",
            "suggested_fix_direction",
        ],
    );
    object_schema(
        json!({"findings": {"type": "array", "items": finding}}),
        vec!["findings"],
    )
}

pub fn repair_schema() -> Value {
    object_schema(
        json!({
            "artifact": {"type": "string"},
            "iteration": {"type": "integer"},
            "changes_made": {"type": "array", "items": {"type": "string"}},
            "unresolved_items": {"type": "array", "items": {"type": "string"}},
            "updated_artifact_path": {"type": "string"}
        }),
        vec![
            "artifact",
            "iteration",
            "changes_made",
            "unresolved_items",
            "updated_artifact_path",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_schema_requires_findings() {
        let schema = review_schema();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"], json!(["findings"]));
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn repair_schema_requires_updated_artifact_path() {
        let schema = repair_schema();
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("updated_artifact_path"))
        );
        assert_eq!(schema["properties"]["changes_made"]["type"], "array");
    }
}

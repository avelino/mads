use schemars::{JsonSchema, generate::SchemaSettings};
use serde_json::Value;

/// JSON Schema for tool arguments in the subset every provider accepts:
/// nested types inlined, no `$ref`, no `format`, no title.
pub fn schema_for<T: JsonSchema>() -> Value {
    let generator = SchemaSettings::draft2020_12()
        .with(|s| {
            s.inline_subschemas = true;
            s.meta_schema = None;
        })
        .into_generator();
    let mut value =
        serde_json::to_value(generator.into_root_schema_for::<T>()).unwrap_or(Value::Null);
    strip(&mut value);
    value
}

fn strip(v: &mut Value) {
    match v {
        Value::Object(map) => {
            for key in ["$schema", "title", "format"] {
                map.remove(key);
            }
            map.values_mut().for_each(strip);
        }
        Value::Array(items) => items.iter_mut().for_each(strip),
        _ => {}
    }
}

const FORBIDDEN_KEYS: &[&str] = &[
    "$ref",
    "$defs",
    "definitions",
    "oneOf",
    "anyOf",
    "allOf",
    "not",
];

/// Paths of constructs some providers reject: references, unions and type arrays (nullable).
pub fn non_portable_keywords(schema: &Value) -> Vec<String> {
    let mut found = Vec::new();
    walk(schema, "$", &mut found);
    found
}

fn walk(v: &Value, path: &str, found: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, child) in map {
                let here = format!("{path}.{k}");
                if FORBIDDEN_KEYS.contains(&k.as_str()) || (k == "type" && child.is_array()) {
                    found.push(here.clone());
                }
                walk(child, &here, found);
            }
        }
        Value::Array(items) => items
            .iter()
            .enumerate()
            .for_each(|(i, c)| walk(c, &format!("{path}[{i}]"), found)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Inner {
        text: String,
        #[serde(default)]
        count: usize,
    }

    #[derive(Deserialize, JsonSchema)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code)]
    struct Outer {
        items: Vec<Inner>,
        #[serde(default)]
        ratio: f64,
    }

    #[test]
    fn nested_types_are_inlined_and_formats_stripped() {
        let s = schema_for::<Outer>();
        assert!(non_portable_keywords(&s).is_empty(), "{s}");
        assert_eq!(s["type"], "object");
        assert_eq!(s["additionalProperties"], false);
        assert_eq!(
            s["properties"]["items"]["items"]["properties"]["text"]["type"],
            "string"
        );
        assert!(s.to_string().find("\"format\"").is_none());
    }

    #[test]
    fn detector_flags_refs_unions_and_nullable() {
        let bad = serde_json::json!({"properties": {"a": {"$ref": "#/x"}, "b": {"anyOf": []}, "c": {"type": ["string", "null"]}}});
        assert_eq!(non_portable_keywords(&bad).len(), 3);
    }
}

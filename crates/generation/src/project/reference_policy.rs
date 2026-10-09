//! Keep literal examples/defaults/extensions out of the generic library's ref traversal.
use super::*;
use std::collections::BTreeSet;

pub(crate) fn dictionary_member(key: &str) -> bool {
    [
        "schemas",
        "properties",
        "patternProperties",
        "$defs",
        "definitions",
        "paths",
        "content",
        "responses",
        "headers",
        "parameters",
        "requestBodies",
        "securitySchemes",
        "links",
        "callbacks",
        "examples",
    ]
    .contains(&key)
}
pub(crate) fn literal_member(key: &str, value: &Value, dictionary: bool) -> bool {
    !dictionary
        && (key.starts_with("x-")
            || ["example", "default", "enum", "const", "value", "mapping"].contains(&key)
            || key == "examples" && value.is_array())
}
pub(crate) fn root_dictionary(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        !object.is_empty()
            && object.values().all(Value::is_object)
            && !object.keys().any(|key| {
                [
                    "openapi",
                    "$ref",
                    "type",
                    "properties",
                    "allOf",
                    "oneOf",
                    "anyOf",
                    "items",
                    "get",
                    "post",
                    "put",
                    "delete",
                    "patch",
                    "responses",
                    "requestBody",
                    "parameters",
                ]
                .contains(&key.as_str())
            })
    })
}
pub(crate) struct LiteralValues {
    values: BTreeMap<String, Value>,
    strings: BTreeSet<String>,
}
impl LiteralValues {
    pub(crate) fn new(documents: impl Iterator<Item = Value>) -> Self {
        let mut strings = BTreeSet::new();
        let mut pending = documents.collect::<Vec<_>>();
        while let Some(value) = pending.pop() {
            match value {
                Value::String(value) => {
                    strings.insert(value);
                }
                Value::Object(object) => pending.extend(object.into_values()),
                Value::Array(array) => pending.extend(array),
                _ => {}
            }
        }
        Self {
            values: BTreeMap::new(),
            strings,
        }
    }
    pub(crate) fn mask(&mut self, value: &mut Value, dictionary: bool) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    if literal_member(key, value, dictionary) {
                        let mut counter = self.values.len();
                        let marker = loop {
                            let marker = format!("__moleapi_source_literal_{counter}__");
                            if self.strings.insert(marker.clone()) {
                                break marker;
                            }
                            counter += 1;
                        };
                        let literal = std::mem::replace(value, Value::String(marker.clone()));
                        self.values.insert(marker, literal);
                    } else {
                        self.mask(value, !dictionary && dictionary_member(key));
                    }
                }
            }
            Value::Array(array) => {
                for value in array {
                    self.mask(value, false);
                }
            }
            _ => {}
        }
    }
    pub(crate) fn restore(&self, value: &mut Value) {
        match value {
            Value::String(marker) => {
                if let Some(literal) = self.values.get(marker) {
                    *value = literal.clone();
                }
            }
            Value::Object(object) => {
                for value in object.values_mut() {
                    self.restore(value);
                }
            }
            Value::Array(array) => {
                for value in array {
                    self.restore(value);
                }
            }
            _ => {}
        }
    }
}

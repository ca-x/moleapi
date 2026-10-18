//! Resolve one saved OpenAPI response into the bounded explicit Mock schema profile.
use crate::{Specification, validate_mock_schema};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Map, Value};
use std::collections::HashMap;

pub struct OpenapiMockIndex {
    document: Value,
}

pub struct OpenapiMockResponse {
    pub schema: String,
    pub method: String,
    pub status: u16,
    pub media_type: String,
}

impl OpenapiMockIndex {
    pub fn parse(spec: &Specification) -> Result<Self> {
        ensure!(spec.kind == "openapi", "Linked Mock source is not OpenAPI");
        ensure!(
            spec.source.len() <= crate::MAX_BODY,
            "OpenAPI source exceeds 5 MiB"
        );
        let document: Value =
            serde_yaml_ng::from_str(&spec.source).context("Invalid linked OpenAPI source")?;
        let version = document
            .get("openapi")
            .and_then(Value::as_str)
            .context("Missing OpenAPI version")?;
        ensure!(
            version.starts_with("3.0.") || version.starts_with("3.1."),
            "Linked Mock requires OpenAPI 3.0 or 3.1"
        );
        ensure!(
            document.get("paths").and_then(Value::as_object).is_some(),
            "OpenAPI paths are missing"
        );
        Ok(Self { document })
    }

    pub fn schema(&self, pointer: &str) -> Result<String> {
        self.response_schema(pointer)
            .map(|response| response.schema)
    }

    pub fn response_schema(&self, pointer: &str) -> Result<OpenapiMockResponse> {
        ensure!(
            pointer.len() <= 4096,
            "Linked Mock schema pointer exceeds 4096 bytes"
        );
        let path = pointer
            .strip_prefix("#/")
            .context("Linked Mock schema pointer must be local")?;
        let parts: Vec<String> = path.split('/').map(unescape).collect::<Result<_>>()?;
        ensure!(
            parts.len() == 8
                && parts[0] == "paths"
                && parts[3] == "responses"
                && parts[5] == "content"
                && parts[7] == "schema",
            "Linked Mock pointer must select a response media schema"
        );
        ensure!(
            matches!(
                parts[2].as_str(),
                "get" | "post" | "put" | "patch" | "delete" | "head" | "options"
            ),
            "Linked Mock operation method is unsupported"
        );
        let status: u16 = parts[4]
            .parse()
            .context("Linked Mock response status must be numeric")?;
        ensure!(
            (200..=599).contains(&status),
            "Linked Mock response status must be 200–599"
        );
        let mime: mime::Mime = parts[6]
            .parse()
            .context("Invalid linked Mock response media type")?;
        ensure!(
            mime.essence_str() == "application/json" || mime.suffix() == Some(mime::JSON),
            "Linked Mock response must use JSON media"
        );
        let operation = self.document["paths"]
            .get(&parts[1])
            .and_then(|item| item.get(&parts[2]))
            .context("Linked Mock operation is missing")?;
        let response = operation["responses"]
            .get(&parts[4])
            .context("Linked Mock response status is missing")?;
        let mut resolver = Resolver::new(&self.document);
        let response = resolver.response(response, 0)?;
        let raw = response["content"]
            .get(&parts[6])
            .and_then(|media| media.get("schema"))
            .context("Linked Mock JSON schema is missing")?;
        let mut schema = if let Some(reference) = raw.get("$ref").and_then(Value::as_str) {
            ensure!(
                raw.as_object().is_some_and(|object| object.len() == 1),
                "Linked Mock root reference siblings are unsupported"
            );
            resolver.check_ref(reference, "#/components/schemas/")?;
            resolver.stack.push(reference.to_owned());
            let target = self
                .document
                .pointer(&reference[1..])
                .context("Unresolved linked Mock schema reference")?;
            let result = resolver.normalize(target, 0);
            resolver.stack.pop();
            result?
        } else {
            resolver.normalize(raw, 0)?
        };
        if !resolver.definitions.is_empty() {
            let object = schema
                .as_object_mut()
                .context("Linked Mock schema root must be an object")?;
            ensure!(
                !object.contains_key("$defs"),
                "Linked Mock root definitions conflict"
            );
            object.insert("$defs".into(), Value::Object(resolver.definitions));
        }
        let source = serde_json::to_string(&schema)?;
        validate_mock_schema(&source)?;
        Ok(OpenapiMockResponse {
            schema: source,
            method: parts[2].to_ascii_uppercase(),
            status,
            media_type: parts[6].clone(),
        })
    }
}

fn unescape(segment: &str) -> Result<String> {
    let mut result = String::with_capacity(segment.len());
    let mut chars = segment.chars();
    while let Some(character) = chars.next() {
        if character == '~' {
            result.push(match chars.next() {
                Some('0') => '~',
                Some('1') => '/',
                _ => bail!("Invalid linked Mock JSON pointer escape"),
            });
        } else {
            result.push(character);
        }
    }
    Ok(result)
}

struct Resolver<'a> {
    document: &'a Value,
    definitions: Map<String, Value>,
    names: HashMap<String, String>,
    stack: Vec<String>,
    nodes: usize,
}

impl<'a> Resolver<'a> {
    fn new(document: &'a Value) -> Self {
        Self {
            document,
            definitions: Map::new(),
            names: HashMap::new(),
            stack: Vec::new(),
            nodes: 0,
        }
    }

    fn step(&mut self, depth: usize) -> Result<()> {
        self.nodes += 1;
        ensure!(
            depth <= 64 && self.nodes <= 10_000,
            "Linked Mock schema complexity limit exceeded"
        );
        Ok(())
    }

    fn check_ref(&self, reference: &str, prefix: &str) -> Result<()> {
        ensure!(
            reference.starts_with(prefix) && reference.len() <= 4096 && !reference.contains('%'),
            "Linked Mock reference must be local under {prefix}"
        );
        ensure!(
            !self.stack.iter().any(|seen| seen == reference),
            "Cyclic linked Mock reference"
        );
        Ok(())
    }

    fn response(&mut self, value: &'a Value, depth: usize) -> Result<&'a Value> {
        self.step(depth)?;
        if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
            self.check_ref(reference, "#/components/responses/")?;
            let target = self
                .document
                .pointer(&reference[1..])
                .context("Unresolved linked Mock response reference")?;
            self.stack.push(reference.to_owned());
            let result = self.response(target, depth + 1);
            self.stack.pop();
            result
        } else {
            Ok(value)
        }
    }

    fn normalize(&mut self, schema: &Value, depth: usize) -> Result<Value> {
        self.step(depth)?;
        let source = schema
            .as_object()
            .context("Linked Mock schema node is not an object")?;
        if let Some(reference) = source.get("$ref").and_then(Value::as_str) {
            ensure!(
                source.len() == 1,
                "Linked Mock reference siblings are unsupported"
            );
            self.check_ref(reference, "#/components/schemas/")?;
            if let Some(name) = self.names.get(reference) {
                return Ok(serde_json::json!({"$ref":format!("#/$defs/{name}")}));
            }
            let target = self
                .document
                .pointer(&reference[1..])
                .context("Unresolved linked Mock schema reference")?;
            let name = format!("ref{}", self.names.len() + 1);
            self.stack.push(reference.to_owned());
            let normalized = self.normalize(target, depth + 1);
            self.stack.pop();
            self.definitions.insert(name.clone(), normalized?);
            self.names.insert(reference.to_owned(), name.clone());
            return Ok(serde_json::json!({"$ref":format!("#/$defs/{name}")}));
        }
        let mut output = Map::new();
        for (key, value) in source {
            match key.as_str() {
                "properties" => {
                    let properties = value
                        .as_object()
                        .context("Linked Mock properties must be an object")?;
                    let mut mapped = Map::new();
                    for (name, child) in properties {
                        mapped.insert(name.clone(), self.normalize(child, depth + 1)?);
                    }
                    output.insert(key.clone(), Value::Object(mapped));
                }
                "items" => {
                    output.insert(
                        key.clone(),
                        if value.is_boolean() {
                            value.clone()
                        } else {
                            self.normalize(value, depth + 1)?
                        },
                    );
                }
                "prefixItems" | "oneOf" | "anyOf" | "allOf" => {
                    let branches = value
                        .as_array()
                        .with_context(|| format!("Linked Mock {key} must be an array"))?;
                    let mut mapped = Vec::with_capacity(branches.len());
                    for branch in branches {
                        mapped.push(self.normalize(branch, depth + 1)?);
                    }
                    output.insert(key.clone(), Value::Array(mapped));
                }
                "type" => {
                    output.insert(key.clone(), value.clone());
                }
                "format" => {
                    let format = value.as_str().context("Invalid linked Mock format")?;
                    match format {
                        "uuid" | "email" | "date" | "date-time" | "uri" => {
                            output.insert("format".into(), value.clone());
                        }
                        _ => bail!("Unsupported linked Mock format {format}"),
                    }
                }
                "nullable" | "readOnly" | "deprecated" => {
                    ensure!(value.is_boolean(), "Invalid linked Mock {key}");
                }
                "example" => {
                    if !source.contains_key("examples") {
                        output.insert("examples".into(), Value::Array(vec![value.clone()]));
                    }
                }
                "title" | "description" | "required" | "enum" | "const" | "default"
                | "examples" | "minimum" | "maximum" | "minLength" | "maxLength" | "minItems"
                | "maxItems" => {
                    output.insert(key.clone(), value.clone());
                }
                _ => bail!("Unsupported linked Mock schema keyword {key}"),
            }
        }
        Ok(Value::Object(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Specification, generate_mock_schema};

    #[test]
    fn linked_response_uses_local_component_schema_without_operation_id() {
        let source = r##"{"openapi":"3.1.0","info":{"title":"Demo","version":"1"},"paths":{"/pets":{"get":{"responses":{"200":{"content":{"application/json":{"schema":{"type":"object","properties":{"id":{"$ref":"#/components/schemas/Id"}},"required":["id"]}}}}}}}},"components":{"schemas":{"Id":{"type":"integer","minimum":3}}}}"##;
        let spec = Specification {
            id: "s".into(),
            name: "S".into(),
            kind: "openapi".into(),
            source: source.into(),
            dialect: "3.1.0".into(),
        };
        let index = OpenapiMockIndex::parse(&spec).unwrap();
        let schema = index
            .schema("#/paths/~1pets/get/responses/200/content/application~1json/schema")
            .unwrap();
        assert_eq!(generate_mock_schema(&schema).unwrap(), r#"{"id":3}"#);
    }

    #[test]
    fn yaml_response_reference_and_nullable_uuid_use_current_source() {
        let source = r#"openapi: 3.0.3
paths:
  /id:
    get:
      responses:
        '200':
          $ref: '#/components/responses/Ok'
components:
  responses:
    Ok:
      content:
        application/json:
          schema:
            type: object
            properties:
              id:
                $ref: '#/components/schemas/Id'
            required: [id]
  schemas:
    Id:
      type: string
      format: uuid
      nullable: true
"#;
        let spec = Specification {
            id: "s".into(),
            name: "S".into(),
            kind: "openapi".into(),
            source: source.into(),
            dialect: "3.0.3".into(),
        };
        let selected = OpenapiMockIndex::parse(&spec)
            .unwrap()
            .response_schema("#/paths/~1id/get/responses/200/content/application~1json/schema")
            .unwrap();
        assert_eq!(
            (
                selected.method.as_str(),
                selected.status,
                selected.media_type.as_str()
            ),
            ("GET", 200, "application/json")
        );
        assert_eq!(
            generate_mock_schema(&selected.schema).unwrap(),
            r#"{"id":"00000000-0000-4000-8000-000000000000"}"#
        );
    }

    #[test]
    fn invalid_pointer_external_and_cyclic_references_reject() {
        let mut document = serde_json::json!({"openapi":"3.1.0","paths":{"/id":{"get":{"responses":{"200":{"content":{"application/json":{"schema":{"$ref":"https://example.test/schema"}}}}}}}}});
        let pointer = "#/paths/~1id/get/responses/200/content/application~1json/schema";
        let as_spec = |value: &Value| Specification {
            id: "s".into(),
            name: "S".into(),
            kind: "openapi".into(),
            source: value.to_string(),
            dialect: "3.1.0".into(),
        };
        assert!(
            OpenapiMockIndex::parse(&as_spec(&document))
                .unwrap()
                .schema(pointer)
                .is_err()
        );
        document["components"] = serde_json::json!({"schemas":{"A":{"$ref":"#/components/schemas/B"},"B":{"$ref":"#/components/schemas/A"}}});
        document["paths"]["/id"]["get"]["responses"]["200"]["content"]["application/json"]["schema"] =
            serde_json::json!({"$ref":"#/components/schemas/A"});
        let index = OpenapiMockIndex::parse(&as_spec(&document)).unwrap();
        assert!(index.schema(pointer).is_err());
        assert!(
            index
                .schema("#/paths/~1id/get/responses/201/content/application~1json/schema")
                .is_err()
        );
        assert!(
            index
                .schema("#/paths/~1id/get/responses/200/content/text~1plain/schema")
                .is_err()
        );
    }

    #[test]
    fn root_reference_siblings_and_numeric_formats_reject_without_loss() {
        let pointer = "#/paths/~1id/get/responses/200/content/application~1json/schema";
        let mut document = serde_json::json!({"openapi":"3.1.0","paths":{"/id":{"get":{"responses":{"200":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Id","minLength":5}}}}}}}},"components":{"schemas":{"Id":{"type":"string"}}}});
        let spec = |value: &Value| Specification {
            id: "s".into(),
            name: "S".into(),
            kind: "openapi".into(),
            source: value.to_string(),
            dialect: "3.1.0".into(),
        };
        assert!(
            OpenapiMockIndex::parse(&spec(&document))
                .unwrap()
                .schema(pointer)
                .is_err()
        );
        document["paths"]["/id"]["get"]["responses"]["200"]["content"]["application/json"]["schema"] =
            serde_json::json!({"type":"integer","format":"int32"});
        assert!(
            OpenapiMockIndex::parse(&spec(&document))
                .unwrap()
                .schema(pointer)
                .is_err()
        );
    }

    #[test]
    fn linked_composition_and_tuple_preserve_component_references() {
        let source = serde_json::json!({"openapi":"3.1.0","paths":{"/items":{"get":{"responses":{"200":{"content":{"application/json":{"schema":{"type":"array","prefixItems":[{"oneOf":[{"$ref":"#/components/schemas/Name"},{"type":"integer","minimum":5}]}],"items":false,"minItems":1}}}}}}}},"components":{"schemas":{"Name":{"type":["null","string"],"minLength":2}}}});
        let spec = Specification {
            id: "s".into(),
            name: "S".into(),
            kind: "openapi".into(),
            source: source.to_string(),
            dialect: "3.1.0".into(),
        };
        let selected = OpenapiMockIndex::parse(&spec)
            .unwrap()
            .schema("#/paths/~1items/get/responses/200/content/application~1json/schema")
            .unwrap();
        assert_eq!(generate_mock_schema(&selected).unwrap(), r#"["xx"]"#);
    }
}

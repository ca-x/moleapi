//! GraphQL syntax and schema handling are delegated to mature parser/Cynic models.
use crate::{MAX_BODY, Protocol, RequestSpec, Specification};
use anyhow::{Context, Result, bail, ensure};
use async_graphql_parser::{
    parse_query, parse_schema,
    types::{BaseType, OperationType, Type},
};
use cynic::QueryBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_GRAPHQL_DOCUMENT: usize = 256 * 1024;
pub const MAX_GRAPHQL_TYPES: usize = 10_000;
pub const MAX_GRAPHQL_NESTING: usize = 32;
const MAX_GRAPHQL_TOKENS: usize = 1_000_000;
/// Quota over mature lexer tokens, before any recursive parser/AST conversion.
/// Strings, escaped strings, block strings and comments are already indivisible tokens.
fn check_graphql_structure(source: &str) -> Result<()> {
    use apollo_parser::TokenKind;
    let mut depth = 0usize;
    for token in apollo_parser::Lexer::new(source).with_limit(MAX_GRAPHQL_TOKENS) {
        let token = token.map_err(|error| anyhow::anyhow!("Invalid GraphQL token: {error}"))?;
        match token.kind() {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LCurly => {
                depth += 1;
                ensure!(
                    depth <= MAX_GRAPHQL_NESTING,
                    "GraphQL structural nesting exceeds {MAX_GRAPHQL_NESTING}"
                );
            }
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RCurly => {
                depth = depth.saturating_sub(1)
            }
            _ => {}
        }
    }
    Ok(())
}
fn bounded_schema(source: &str) -> Result<async_graphql_parser::types::ServiceDocument> {
    ensure!(source.len() <= MAX_BODY, "GraphQL schema exceeds 5 MiB");
    check_graphql_structure(source)?;
    parse_schema(source).map_err(|error| {
        if let async_graphql_parser::Error::Syntax { start, .. } = &error {
            let failing_line = source.lines().nth(start.line.saturating_sub(1));
            let failing_source = failing_line.and_then(|line| {
                line.char_indices().nth(start.column.saturating_sub(1)).map(|(offset,_)| &line[offset..])
            });
            let failing_token = failing_source.and_then(|rest| apollo_parser::Lexer::new(rest).next()).and_then(Result::ok);
            if failing_token.is_some_and(|token| token.kind() == apollo_parser::TokenKind::Name && token.data() == "DIRECTIVE_DEFINITION") {
                return anyhow::anyhow!("Unsupported GraphQL SDL directive location DIRECTIVE_DEFINITION; original source is retained");
            }
        }
        anyhow::Error::new(error).context("Invalid GraphQL SDL")
    })
}
/// Build a schema definition using the existing Cynic AST writer/SDL printer.
fn explicit_schema_roots(schema: &cynic_introspection::Schema) -> String {
    use cynic_parser::{
        common::OperationType,
        type_system::{
            storage::{RootOperationTypeDefinitionRecord, SchemaDefinitionRecord},
            writer::TypeSystemAstWriter,
        },
    };
    let mut writer = TypeSystemAstWriter::new();
    let mut roots = Vec::new();
    for (operation_type, name) in [
        (OperationType::Query, Some(schema.query_type.as_str())),
        (OperationType::Mutation, schema.mutation_type.as_deref()),
        (
            OperationType::Subscription,
            schema.subscription_type.as_deref(),
        ),
    ] {
        if let Some(name) = name {
            roots.push(RootOperationTypeDefinitionRecord {
                operation_type,
                operation_type_span: Default::default(),
                named_type: writer.intern_string(name),
                named_type_span: Default::default(),
                span: Default::default(),
            });
        }
    }
    let root_operations = writer.root_operation_definitions(roots);
    writer.schema_definition(SchemaDefinitionRecord {
        description: None,
        directives: Default::default(),
        root_operations,
        span: Default::default(),
    });
    writer.finish().to_sdl_pretty()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphqlPayload {
    pub query: String,
    #[serde(default = "object")]
    pub variables: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
}
fn object() -> Value {
    serde_json::json!({})
}
impl GraphqlPayload {
    pub fn selected_operation(&self) -> Result<OperationType> {
        ensure!(
            self.query.len() <= MAX_GRAPHQL_DOCUMENT,
            "GraphQL document exceeds 256 KiB"
        );
        ensure!(
            self.variables.is_object(),
            "GraphQL variables must be a JSON object"
        );
        ensure!(
            serde_json::to_vec(&self.variables)?.len() <= MAX_BODY,
            "GraphQL variables exceed 5 MiB"
        );
        check_graphql_structure(&self.query)?;
        let doc = parse_query(&self.query).context("Invalid GraphQL document")?;
        let operations: Vec<_> = doc.operations.iter().collect();
        let selected = if let Some(name) = &self.operation_name {
            operations
                .iter()
                .find(|(n, _)| n.is_some_and(|n| n.as_str() == name))
                .context("GraphQL operation name not found")?
                .1
        } else {
            ensure!(
                operations.len() == 1,
                "Select an operation name for a document with multiple operations"
            );
            operations[0].1
        };
        for variable in &selected.node.variable_definitions {
            let definition = &variable.node;
            let value = self.variables.get(definition.name.node.as_str());
            if let Some(value) = value {
                validate_variable(value, &definition.var_type.node)?;
            } else {
                ensure!(
                    definition.default_value().is_some(),
                    "Missing required GraphQL variable ${}",
                    definition.name.node
                );
            }
        }
        Ok(selected.node.ty)
    }
}
fn validate_variable(value: &Value, ty: &Type) -> Result<()> {
    if value.is_null() {
        ensure!(ty.nullable, "Non-null GraphQL variable cannot be null");
        return Ok(());
    }
    match &ty.base {
        BaseType::List(item) => {
            if let Some(values) = value.as_array() {
                for value in values {
                    validate_variable(value, item)?;
                }
            } else {
                validate_variable(value, item)?;
            }
        }
        BaseType::Named(name) => {
            let valid = match name.as_str() {
                "Int" => value.as_i64().is_some_and(|n| i32::try_from(n).is_ok()),
                "Float" => value.is_number(),
                "String" => value.is_string(),
                "Boolean" => value.is_boolean(),
                "ID" => value.is_string() || value.as_i64().is_some(),
                _ => true, // Custom scalar/enum/input coercion is the target schema's authority.
            };
            ensure!(valid, "Invalid GraphQL variable value for {ty}");
        }
    }
    Ok(())
}
pub fn graphql_payload(request: &RequestSpec) -> Result<GraphqlPayload> {
    match &request.protocol {
        Protocol::Graphql {
            document,
            variables,
            operation_name,
            variables_source,
            ..
        } => Ok(GraphqlPayload {
            query: document.clone(),
            variables: match variables_source {
                Some(source) => {
                    serde_json::from_str(source).context("Invalid GraphQL variables JSON")?
                }
                None => variables.as_ref().clone(),
            },
            operation_name: operation_name.clone(),
        }),
        _ => bail!("Request is not GraphQL"),
    }
}
pub fn validate_graphql_draft(request: &RequestSpec, templates: bool) -> Result<()> {
    if let Protocol::Graphql {
        document,
        variables,
        variables_source,
        connection_params,
        subscription_url,
        ..
    } = &request.protocol
    {
        if let Some(source) = variables_source {
            ensure!(
                source.len() <= MAX_BODY,
                "GraphQL variables draft exceeds 5 MiB"
            );
        }
        ensure!(
            document.len() <= MAX_GRAPHQL_DOCUMENT,
            "GraphQL document exceeds 256 KiB"
        );
        ensure!(
            variables.is_object(),
            "GraphQL variables must be a JSON object"
        );
        ensure!(
            connection_params.is_object(),
            "GraphQL connection parameters must be a JSON object"
        );
        ensure!(
            serde_json::to_vec(connection_params)?.len() <= MAX_GRAPHQL_DOCUMENT,
            "GraphQL connection parameters exceed 256 KiB"
        );
        if let Some(url) = subscription_url
            && (!templates || !url.contains("{{"))
        {
            crate::protocol_url(url, true)?;
        }
        // Saved drafts may be incomplete; execution paths validate the selected AST.
    }
    Ok(())
}
/// Produce the actual HTTP envelope before pre scripts inspect/edit pm.request.body.
pub fn prepare_graphql(request: &RequestSpec) -> Result<RequestSpec> {
    if !request.protocol.is_graphql() {
        return Ok(request.clone());
    }
    validate_graphql_draft(request, true)?;
    let mut request = request.clone();
    request.method = "POST".into();
    request.body_kind = "json".into();
    let payload = graphql_payload(&request)?;
    ensure!(
        payload.variables.is_object(),
        "GraphQL variables must be a JSON object"
    );
    request.body = serde_json::to_string(&payload)?;
    Ok(request)
}
/// Revalidate the post-script envelope and keep typed state in agreement with wire bytes.
pub fn reconcile_graphql(request: &mut RequestSpec) -> Result<()> {
    if !request.protocol.is_graphql() {
        return Ok(());
    }
    ensure!(
        request.method == "POST" && request.body_kind == "json",
        "GraphQL requires POST with a JSON GraphQL envelope; pre-script method/body-mode changes are unsupported"
    );
    let payload: GraphqlPayload =
        serde_json::from_str(&request.body).context("Invalid GraphQL request envelope")?;
    payload.selected_operation()?;
    if let Protocol::Graphql {
        document,
        variables,
        operation_name,
        variables_source,
        ..
    } = &mut request.protocol
    {
        *document = payload.query;
        *variables_source = None; // Execution clone uses the script-edited, resolved envelope.
        **variables = payload.variables;
        *operation_name = payload.operation_name;
    }
    Ok(())
}
pub fn graphql_is_subscription(request: &RequestSpec) -> Result<bool> {
    Ok(graphql_payload(request)?.selected_operation()? == OperationType::Subscription)
}
pub fn introspection_payload() -> GraphqlPayload {
    let operation = cynic_introspection::IntrospectionQuery::build(());
    GraphqlPayload {
        query: operation.query,
        variables: object(),
        operation_name: None,
    }
}
pub fn graphql_schema_sdl(specification: &Specification) -> Result<String> {
    ensure!(
        specification.source.len() <= MAX_BODY,
        "GraphQL schema exceeds 5 MiB"
    );
    match specification.kind.as_str() {
        "graphql-sdl" => {
            let schema = bounded_schema(&specification.source)?;
            ensure!(
                schema.definitions.len() <= MAX_GRAPHQL_TYPES,
                "GraphQL schema exceeds 10000 definitions"
            );
            Ok(specification.source.clone())
        }
        "graphql-introspection" => {
            let response: Value = serde_json::from_str(&specification.source)
                .context("Invalid GraphQL introspection JSON")?;
            let data = response.get("data").unwrap_or(&response);
            let types = data
                .pointer("/__schema/types")
                .and_then(Value::as_array)
                .context("GraphQL introspection has no schema types")?;
            ensure!(
                types.len() <= MAX_GRAPHQL_TYPES,
                "GraphQL schema exceeds 10000 types"
            );
            let query: cynic_introspection::IntrospectionQuery =
                serde_json::from_value(data.clone())
                    .context("Invalid GraphQL introspection model")?;
            let schema = query
                .into_schema()
                .context("Invalid GraphQL introspection schema")?;
            // The mature SDL parser predates this new custom-directive location.
            // Built-in directives are supplied by schema consumers and omitted by Cynic's writer.
            for directive in &schema.directives {
                if !matches!(
                    directive.name.as_str(),
                    "skip" | "include" | "deprecated" | "specifiedBy"
                ) {
                    ensure!(
                        !directive
                            .locations
                            .contains(&cynic_introspection::DirectiveLocation::DirectiveDefinition),
                        "Unsupported GraphQL SDL directive location DIRECTIVE_DEFINITION for @{}; original introspection metadata is retained",
                        directive.name
                    );
                }
            }
            let mut sdl = schema.to_sdl();
            let parsed = bounded_schema(&sdl)?;
            // Conventional type names do not imply actual operation roots after introspection.
            if !parsed.definitions.iter().any(|definition| {
                matches!(
                    definition,
                    async_graphql_parser::types::TypeSystemDefinition::Schema(_)
                )
            }) {
                sdl = format!("{}\n{sdl}", explicit_schema_roots(&schema));
            }
            ensure!(
                sdl.len() <= MAX_BODY,
                "Generated GraphQL schema exceeds 5 MiB"
            );
            bounded_schema(&sdl).context("Invalid introspected GraphQL SDL")?;
            Ok(sdl)
        }
        _ => bail!("Specification is not a GraphQL schema"),
    }
}

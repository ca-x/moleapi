//! Owner-scoped GraphQL schema operations through the shared execution pipeline.
use crate::{ApiError, AppState, auth::Identity, execution, privacy::Redactor, workspaces::owned};
use axum::{Extension, Json, extract::State};
use moleapi_core::{Pair, Protocol, RequestSpec, Response, Specification, VariableUpdate};
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
pub struct Introspect {
    workspace_id: String,
    request: RequestSpec,
    environment_id: Option<String>,
    #[serde(default)]
    variables: Vec<Pair>,
    #[serde(default)]
    data: Vec<Pair>,
    #[serde(default)]
    locals: Vec<VariableUpdate>,
}
#[derive(Serialize)]
pub struct IntrospectionResult {
    response: Response,
    #[serde(skip_serializing_if = "Option::is_none")]
    specification: Option<Specification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sdl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}
pub async fn introspect(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Introspect>,
) -> Result<Json<IntrospectionResult>, ApiError> {
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let collection = w
        .data
        .collections
        .iter()
        .find(|col| col.requests.iter().any(|r| r.id == c.request.id))
        .ok_or_else(ApiError::not_found)?;
    let e = execution::environment(&w, c.environment_id.as_deref())?;
    let mut scopes = execution::variables(
        &s,
        &w,
        Some(collection),
        e,
        &c.data,
        &c.variables,
        &c.locals,
    )?;
    let mut request = c.request;
    let payload = moleapi_core::introspection_payload();
    request.protocol = Protocol::Graphql {
        document: payload.query,
        variables: Box::new(payload.variables),
        variables_source: None,
        operation_name: None,
        connection_params: serde_json::json!({}),
        subscription_url: None,
    };
    let response =
        execution::perform(&s, &owner.0, &w, &request, Some(collection), &mut scopes).await?;
    let mut result = IntrospectionResult {
        response,
        specification: None,
        sdl: None,
        error: None,
    };
    // Target failures remain a bounded actual response inside a successful MoleAPI call.
    // A target401 never represents expiration of the MoleAPI authentication session.
    if !(200..300).contains(&result.response.status) || result.response.truncated {
        result.error = Some(format!(
            "Introspection endpoint returned HTTP {}{}",
            result.response.status,
            if result.response.truncated {
                " (response truncated)"
            } else {
                ""
            }
        ));
    } else {
        let redactor = Redactor::new(&scopes.private_values)?;
        let scrub = |text: &str| {
            let mut value = serde_json::json!(text);
            redactor.scrub(&mut value);
            value.as_str().unwrap_or("[REDACTED]").to_owned()
        };
        let spec = Specification {
            id: uuid::Uuid::new_v4().to_string(),
            // This request retains its original title; perform resolves an execution clone.
            name: scrub(&format!("{} schema", request.name)),
            kind: "graphql-introspection".into(),
            source: result.response.body.clone(),
            dialect: "graphql-june2018".into(),
        };
        if scrub(&spec.source) != spec.source
            || serde_json::to_string(&spec).is_ok_and(|text| scrub(&text) != text)
        {
            result.error = Some(
                "Introspection source contains private execution values; source withheld".into(),
            );
        } else {
            match moleapi_core::graphql_schema_sdl(&spec) {
                Ok(sdl) if scrub(&sdl) == sdl => {
                    result.sdl = Some(sdl);
                    result.specification = Some(spec);
                }
                Ok(_) => {
                    result.error = Some(
                        "Introspection schema contains private execution values; source withheld"
                            .into(),
                    )
                }
                Err(error) => result.error = Some(scrub(&error.to_string())),
            }
        }
    }
    Ok(Json(result))
}
#[derive(Deserialize)]
pub struct SchemaSource {
    workspace_id: String,
    specification_id: String,
}
#[derive(Serialize)]
pub struct SchemaResult {
    specification: Specification,
    sdl: String,
}
pub async fn schema(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<SchemaSource>,
) -> Result<Json<SchemaResult>, ApiError> {
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let specification = w
        .data
        .specifications
        .iter()
        .find(|spec| spec.id == c.specification_id)
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    let sdl = moleapi_core::graphql_schema_sdl(&specification)
        .map_err(|error| ApiError::bad(error.to_string()))?;
    Ok(Json(SchemaResult { specification, sdl }))
}

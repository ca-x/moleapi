//! Workspace-owned protobuf and Reflection resources shared by HTTP and offline IPC.
use crate::{ApiError, AppState, auth::Identity, execution, privacy::Redactor, workspaces::owned};
use axum::{Extension, Json, extract::State};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::{GrpcSchema, Protocol, RequestSpec, Specification, VariableUpdate};
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
pub struct SchemaSource {
    workspace_id: String,
    specification_id: String,
}
#[derive(Serialize)]
pub struct SchemaResult {
    specification: Specification,
    schema: GrpcSchema,
}
fn compile(specification: Specification) -> Result<SchemaResult, ApiError> {
    let pool =
        moleapi_core::protobuf_pool(&specification).map_err(|e| ApiError::bad(e.to_string()))?;
    let schema = moleapi_core::grpc_schema(&pool).map_err(|e| ApiError::bad(e.to_string()))?;
    Ok(SchemaResult {
        specification,
        schema,
    })
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
    Ok(Json(compile(specification)?))
}
#[derive(Deserialize)]
pub struct Import {
    workspace_id: String,
    name: String,
    source: String,
}
pub async fn import(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Import>,
) -> Result<Json<SchemaResult>, ApiError> {
    owned(&s, &owner.0, &c.workspace_id).await?;
    Ok(Json(compile(Specification {
        id: uuid::Uuid::new_v4().to_string(),
        name: c.name,
        kind: "protobuf".into(),
        source: c.source,
        dialect: "protobuf".into(),
    })?))
}
#[derive(Deserialize)]
pub struct Reflect {
    workspace_id: String,
    request: RequestSpec,
    environment_id: Option<String>,
    #[serde(default)]
    locals: Vec<VariableUpdate>,
}
#[derive(Serialize)]
pub struct ReflectResult {
    #[serde(flatten)]
    result: moleapi_protocols::ReflectionResult,
    variable_updates: Vec<VariableUpdate>,
    request_updates: Vec<moleapi_core::RequestUpdate>,
    logs: Vec<moleapi_core::ScriptLog>,
    tests: Vec<moleapi_core::TestResult>,
}
pub async fn reflect(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    headers: axum::http::HeaderMap,
    Json(c): Json<Reflect>,
) -> Result<Json<ReflectResult>, ApiError> {
    let admission = s.protocol_admission.owner(&owner.0)?;
    let generation = {
        let guard = admission.lock().await;
        crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
        *guard
    };
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let collection = w
        .data
        .collections
        .iter()
        .find(|col| col.requests.iter().any(|r| r.id == c.request.id))
        .ok_or_else(ApiError::not_found)?;
    let environment = execution::environment(&w, c.environment_id.as_deref())?;
    let mut scopes =
        execution::variables(&s, &w, Some(collection), environment, &[], &[], &c.locals)?;
    let saved_request_name = c.request.name.clone();
    let mut request = c.request;
    // Reflection uses the connection context, independently of an incomplete method draft.
    request.protocol = Protocol::Grpc {
        service: "grpc.reflection.v1.ServerReflection".into(),
        method: "ServerReflectionInfo".into(),
        message_source: "{}".into(),
    };
    let (request, mut feedback, mut variable_updates, mut request_updates) =
        execution::prepare_live(&s, &owner.0, &w, &request, collection, &mut scopes).await?;
    let redactor = Redactor::new(&scopes.private_values)?;
    let scrub = |text: &str| {
        let mut value = serde_json::json!(text);
        redactor.scrub(&mut value);
        value.as_str().unwrap_or("[REDACTED]").to_owned()
    };
    for log in &mut feedback.logs {
        log.message = scrub(&log.message);
    }
    for test in &mut feedback.tests {
        test.name = scrub(&test.name);
        test.actual = scrub(&test.actual);
        test.expected = scrub(&test.expected);
    }
    variable_updates.retain(|u| {
        !matches!(u.scope.as_str(), "local" | "vault")
            && u.value.as_ref().is_none_or(|value| scrub(value) == *value)
    });
    request_updates.retain(|u| scrub(&u.value) == u.value);
    let lease = {
        let guard = admission.lock().await;
        if *guard != generation {
            return Err(ApiError::unauthorized());
        }
        crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
        owned(&s, &owner.0, &c.workspace_id).await?;
        let lease = s
            .protocol_sessions
            .reserve_reflection(
                &owner.0,
                &c.workspace_id,
                &request,
                scrub(&moleapi_core::redact_url(
                    &request.url,
                    Some(&scopes.effective()),
                )),
            )
            .map_err(|error| ApiError {
                status: axum::http::StatusCode::TOO_MANY_REQUESTS,
                message: scrub(&error.to_string()),
            })?;
        // Register first so deletion can cancel this record; then recheck ownership.
        owned(&s, &owner.0, &c.workspace_id).await?;
        lease
    };
    let reflected = tokio::select! {
        biased;
        _ = lease.cancelled() => {
            crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
            owned(&s, &owner.0, &c.workspace_id).await?;
            return Err(ApiError::bad("Reflection cancelled after owner/workspace lifecycle changed"));
        },
        result = moleapi_protocols::reflect(&request, moleapi_core::NetworkPolicy { allow_private_network: s.local || s.config.allow_private_network }) => result,
    };
    let result = match reflected {
        Ok(mut result) => {
            if let Some(specification) = &mut result.specification {
                // Canonical labels belong to shared drafts, never the resolved execution scope.
                specification.name = scrub(&format!("{saved_request_name} schema"));
            }
            // Reflection descriptors can carry server-side options containing sensitive data.
            // Never return a source containing any known private execution values.
            if result.specification.as_ref().is_some_and(|spec| {
                let decoded = serde_json::from_str::<moleapi_core::ProtobufSource>(&spec.source)
                    .ok()
                    .and_then(|source| match source {
                        moleapi_core::ProtobufSource::Descriptor {
                            descriptor_set_base64,
                        } => STANDARD.decode(descriptor_set_base64).ok(),
                        _ => None,
                    });
                let source_private = decoded.as_ref().is_some_and(|bytes| {
                    scopes
                        .private_values
                        .iter()
                        .filter(|value| !value.is_empty())
                        .any(|value| {
                            bytes
                                .windows(value.len())
                                .any(|window| window == value.as_bytes())
                        })
                });
                source_private
                    || scrub(&spec.source) != spec.source
                    || serde_json::to_string(spec).is_ok_and(|text| scrub(&text) != text)
                    || result.schema.as_ref().is_some_and(|schema| {
                        serde_json::to_string(schema).is_ok_and(|text| scrub(&text) != text)
                    })
            }) {
                moleapi_protocols::ReflectionResult {
                    status: None,
                    specification: None,
                    schema: None,
                    error: Some(
                        "Reflection source contains private execution values; source withheld"
                            .into(),
                    ),
                }
            } else {
                result
            }
        }
        Err(error) => moleapi_protocols::ReflectionResult {
            status: error.downcast_ref::<tonic::Status>().map(|status| {
                moleapi_protocols::ReflectionStatus {
                    code: status.code() as u32,
                    name: format!("{:?}", status.code()),
                    message: scrub(status.message()),
                    details_base64: {
                        let text = String::from_utf8_lossy(status.details());
                        if scrub(&text) != text {
                            "[REDACTED]".into()
                        } else {
                            scrub(&STANDARD.encode(status.details()))
                        }
                    },
                }
            }),
            specification: None,
            schema: None,
            error: Some(scrub(&format!("{error:#}"))),
        },
    };
    let guard = admission.lock().await;
    if *guard != generation {
        return Err(ApiError::unauthorized());
    }
    crate::auth::still_authenticated(&s, &owner.0, &headers).await?;
    owned(&s, &owner.0, &c.workspace_id).await?;
    Ok(Json(ReflectResult {
        result,
        variable_updates,
        request_updates,
        logs: feedback.logs,
        tests: feedback.tests,
    }))
}

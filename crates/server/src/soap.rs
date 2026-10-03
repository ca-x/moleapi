//! Owner-bound WSDL candidates. No implicit filesystem or network import resolution.
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{Extension, Json, extract::State};
use moleapi_core::{Pair, SoapSchema, Specification, VariableUpdate};
use serde::{Deserialize, Serialize};
#[derive(Deserialize, Default)]
struct SourceScopes {
    #[serde(default)]
    environment_id: Option<String>,
    #[serde(default)]
    variables: Vec<Pair>,
    #[serde(default)]
    data: Vec<Pair>,
    #[serde(default)]
    locals: Vec<VariableUpdate>,
}
fn source_scopes(
    s: &AppState,
    w: &moleapi_core::Workspace,
    c: &SourceScopes,
) -> Result<moleapi_core::VariableScopes, ApiError> {
    let environment = crate::execution::environment(w, c.environment_id.as_deref())?;
    crate::execution::variables(s, w, None, environment, &c.data, &c.variables, &c.locals)
}
#[derive(Deserialize)]
pub struct Import {
    #[serde(flatten)]
    scopes: SourceScopes,
    workspace_id: String,
    name: String,
    source: String,
}
#[derive(Deserialize)]
pub struct Schema {
    workspace_id: String,
    specification_id: String,
}
#[derive(Serialize)]
pub struct SchemaResult {
    specification: Specification,
    schema: SoapSchema,
}
fn compile(specification: Specification) -> Result<SchemaResult, ApiError> {
    let schema =
        moleapi_core::soap_schema(&specification).map_err(|e| ApiError::bad(e.to_string()))?;
    Ok(SchemaResult {
        specification,
        schema,
    })
}
pub async fn import(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Import>,
) -> Result<Json<SchemaResult>, ApiError> {
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let scopes = source_scopes(&s, &w, &c.scopes)?;
    let candidate = Specification {
        id: uuid::Uuid::new_v4().to_string(),
        name: c.name,
        source: c.source,
        kind: "wsdl".into(),
        dialect: "wsdl1.1".into(),
    };
    screen(&candidate, &scopes.private_values)?;
    Ok(Json(compile(candidate)?))
}
fn screen(
    spec: &Specification,
    values: &std::collections::BTreeSet<String>,
) -> Result<(), ApiError> {
    let mut value = serde_json::to_value(spec).map_err(|_| ApiError::internal())?;
    let before = value.clone();
    crate::privacy::Redactor::new(values)?.scrub(&mut value);
    if before != value {
        return Err(ApiError::bad(
            "WSDL source/name contains private scoped values; candidate withheld",
        ));
    }
    if let Ok(bundle) = serde_json::from_str::<moleapi_core::SoapSource>(&spec.source) {
        if bundle.files.len() > 32
            || bundle.files.iter().map(|f| f.content.len()).sum::<usize>()
                > moleapi_core::MAX_SOAP_SOURCE
        {
            return Err(ApiError::bad("WSDL source exceeds file/byte budget"));
        }
        for file in bundle.files {
            if let Ok(decoded) = moleapi_core::soap_xml_data_values(&file.content) {
                let mut decoded = serde_json::json!(decoded);
                let before = decoded.clone();
                crate::privacy::Redactor::new(values)?.scrub(&mut decoded);
                if before != decoded {
                    return Err(ApiError::bad(
                        "WSDL XML source contains private scoped values; candidate withheld",
                    ));
                }
            }
        }
    }
    Ok(())
}
pub async fn schema(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Schema>,
) -> Result<Json<SchemaResult>, ApiError> {
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let spec = w
        .data
        .specifications
        .iter()
        .find(|v| v.id == c.specification_id)
        .cloned()
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(compile(spec)?))
}
#[derive(Deserialize)]
pub struct ImportUrl {
    #[serde(flatten)]
    scopes: SourceScopes,
    workspace_id: String,
    name: String,
    url: String,
    #[serde(default = "timeout")]
    timeout_ms: u64,
    #[serde(default = "tls")]
    verify_tls: bool,
}
fn timeout() -> u64 {
    30_000
}
fn tls() -> bool {
    true
}
pub async fn import_url(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<ImportUrl>,
) -> Result<Json<SchemaResult>, ApiError> {
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    if !(1..=120_000).contains(&c.timeout_ms) {
        return Err(ApiError::bad("Timeout must be 1..120000ms"));
    }
    let url = moleapi_core::valid_url(&c.url).map_err(|e| ApiError::bad(e.to_string()))?;
    let body = tokio::time::timeout(std::time::Duration::from_millis(c.timeout_ms), async {
        let client = moleapi_core::checked_client(
            &url,
            moleapi_core::NetworkPolicy {
                allow_private_network: s.local || s.config.allow_private_network,
            },
            c.verify_tls,
        )
        .await?;
        let response = client.get(url).send().await?;
        anyhow::ensure!(
            response.status().is_success(),
            "WSDL source returned HTTP {}; redirects require explicit URL selection",
            response.status()
        );
        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            anyhow::ensure!(
                bytes.len() + chunk.len() <= moleapi_core::MAX_SOAP_SOURCE,
                "WSDL source exceeds 2MiB"
            );
            bytes.extend_from_slice(&chunk)
        }
        Ok::<_, anyhow::Error>(String::from_utf8(bytes)?)
    })
    .await
    .map_err(|_| ApiError::bad("WSDL URL import timed out"))?
    .map_err(|e| ApiError::bad(e.to_string()))?;
    let source = serde_json::to_string(&moleapi_core::SoapSource {
        entry_file: "service.wsdl".into(),
        files: vec![moleapi_core::SoapFile {
            path: "service.wsdl".into(),
            content: body,
        }],
    })
    .map_err(|_| ApiError::internal())?;
    let spec = Specification {
        id: uuid::Uuid::new_v4().to_string(),
        name: c.name,
        kind: "wsdl".into(),
        dialect: "wsdl1.1".into(),
        source,
    };
    let scopes = source_scopes(&s, &w, &c.scopes)?;
    screen(&spec, &scopes.private_values)?;
    Ok(Json(compile(spec)?))
}
pub(crate) fn validate_selected(
    data: &moleapi_core::WorkspaceData,
    r: &moleapi_core::RequestSpec,
) -> anyhow::Result<()> {
    let moleapi_core::Protocol::Soap { config } = &r.protocol else {
        return Ok(());
    };
    let Some(id) = &r.specification_id else {
        anyhow::ensure!(
            config.service.is_empty() && config.port.is_empty() && config.operation.is_empty(),
            "SOAP selection requires an attached WSDL specification"
        );
        return Ok(());
    };
    let spec = data
        .specifications
        .iter()
        .find(|s| &s.id == id)
        .ok_or_else(|| anyhow::anyhow!("SOAP specification not found in owned workspace"))?;
    let schema = moleapi_core::soap_schema(spec)?;
    let service = schema
        .services
        .iter()
        .find(|s| s.name == config.service)
        .ok_or_else(|| anyhow::anyhow!("SOAP service not found"))?;
    let port = service
        .ports
        .iter()
        .find(|p| p.name == config.port)
        .ok_or_else(|| anyhow::anyhow!("SOAP port not found"))?;
    let op = port
        .operations
        .iter()
        .find(|o| o.name == config.operation)
        .ok_or_else(|| anyhow::anyhow!("SOAP operation not found"))?;
    anyhow::ensure!(
        config.version == port.version && config.action == op.action,
        "SOAP version/action does not match selected WSDL operation"
    );
    anyhow::ensure!(
        op.binding_supported,
        "Unsupported WSDL binding style/use; document/literal required"
    );
    anyhow::ensure!(
        !op.input_elements.is_empty(),
        "Cannot resolve selected WSDL input elements; use standalone raw SOAP"
    );
    let actual = moleapi_core::parse_bounded_xml(&r.body)?;
    let body = actual
        .root_element()
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "Body")
        .ok_or_else(|| anyhow::anyhow!("SOAP Body missing"))?;
    let actual: Vec<_> = body
        .children()
        .filter(|n| n.is_element())
        .map(|n| (n.tag_name().namespace().unwrap_or(""), n.tag_name().name()))
        .collect();
    let expected: Vec<_> = op
        .input_elements
        .iter()
        .map(|e| (e.namespace.as_str(), e.name.as_str()))
        .collect();
    anyhow::ensure!(
        actual == expected,
        "SOAP payload does not match selected WSDL message elements"
    );
    Ok(())
}
#[derive(Deserialize)]
pub struct TemplateSource {
    workspace_id: String,
    specification_id: String,
    service: String,
    port: String,
    operation: String,
}
#[derive(Serialize)]
pub struct TemplateResult {
    template: String,
    fields: Vec<moleapi_core::SoapField>,
    action: String,
    version: String,
    address: String,
}
pub async fn template(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<TemplateSource>,
) -> Result<Json<TemplateResult>, ApiError> {
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let spec = w
        .data
        .specifications
        .iter()
        .find(|v| v.id == c.specification_id)
        .ok_or_else(ApiError::not_found)?;
    let schema = moleapi_core::soap_schema(spec).map_err(|e| ApiError::bad(e.to_string()))?;
    let service = schema
        .services
        .into_iter()
        .find(|v| v.name == c.service)
        .ok_or_else(|| ApiError::bad("SOAP service not found"))?;
    let port = service
        .ports
        .into_iter()
        .find(|v| v.name == c.port)
        .ok_or_else(|| ApiError::bad("SOAP port not found"))?;
    let operation = port
        .operations
        .into_iter()
        .find(|v| v.name == c.operation)
        .ok_or_else(|| ApiError::bad("SOAP operation not found"))?;
    if let Some(error) = operation.error {
        return Err(ApiError::bad(error));
    }
    Ok(Json(TemplateResult {
        template: operation.template,
        fields: operation.fields,
        action: operation.action,
        version: port.version,
        address: port.address,
    }))
}

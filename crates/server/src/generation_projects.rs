//! Owner/workspace-bound project generation shares hosted and native routes.
use crate::{ApiError, AppState, auth::Identity, workspaces::owned};
use axum::{Extension, Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;
#[derive(Default)]
pub(crate) struct Jobs {
    entries: Mutex<HashMap<String, (String, String, CancellationToken)>>,
    early_cancels: Mutex<HashMap<(String, String, String), std::time::Instant>>,
}
impl Jobs {
    pub(crate) fn cancel_job(
        &self,
        owner: &str,
        workspace: Option<&str>,
        id: &str,
    ) -> Result<(), ApiError> {
        let entries = self.entries.lock().unwrap();
        let Some((job_owner, job_workspace, token)) = entries.get(id) else {
            let workspace = workspace.ok_or_else(ApiError::not_found)?;
            if id.is_empty()
                || id.len() > 128
                || !id
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
            {
                return Err(ApiError::bad("Invalid run job ID"));
            }
            let mut early = self.early_cancels.lock().unwrap();
            early.retain(|_, created| created.elapsed() < std::time::Duration::from_secs(30));
            if early.len() >= 64 {
                return Err(ApiError::bad("Cancellation capacity reached"));
            }
            early.insert(
                (owner.into(), workspace.into(), id.into()),
                std::time::Instant::now(),
            );
            return Ok(());
        };
        if job_owner != owner || workspace.is_some_and(|workspace| workspace != job_workspace) {
            return Err(ApiError::not_found());
        }
        token.cancel();
        Ok(())
    }
    pub(crate) fn start(
        self: &Arc<Self>,
        owner: &str,
        workspace: &str,
        id: &str,
    ) -> Result<Lease, ApiError> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ApiError::bad("Invalid generation job ID"));
        }
        let mut jobs = self.entries.lock().unwrap();
        if jobs.len() >= 32 || jobs.values().filter(|(o, _, _)| o == owner).count() >= 2 {
            return Err(ApiError::bad("Project generation capacity reached"));
        }
        if jobs.contains_key(id) {
            return Err(ApiError::bad("Generation job ID already active"));
        }
        let cancel = CancellationToken::new();
        let mut early = self.early_cancels.lock().unwrap();
        early.retain(|_, created| created.elapsed() < std::time::Duration::from_secs(30));
        if early
            .remove(&(owner.into(), workspace.into(), id.into()))
            .is_some()
        {
            cancel.cancel();
        }
        jobs.insert(id.into(), (owner.into(), workspace.into(), cancel.clone()));
        Ok(Lease {
            hub: self.clone(),
            id: id.into(),
            cancel,
        })
    }
    pub(crate) fn stop_owner(&self, owner: &str) {
        for (o, _, token) in self.entries.lock().unwrap().values() {
            if o == owner {
                token.cancel();
            }
        }
    }
    pub(crate) fn stop_workspace(&self, owner: &str, workspace: &str) {
        for (o, w, token) in self.entries.lock().unwrap().values() {
            if o == owner && w == workspace {
                token.cancel();
            }
        }
    }
}
pub(crate) struct Lease {
    hub: Arc<Jobs>,
    id: String,
    pub(crate) cancel: CancellationToken,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.hub.entries.lock().unwrap().remove(&self.id);
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Generate {
    workspace_id: String,
    specification_id: String,
    job_id: String,
    target: String,
    #[serde(default)]
    options: std::collections::BTreeMap<String, Value>,
    #[serde(default)]
    include_secrets: bool,
    #[serde(default)]
    templates: Option<moleapi_generation::project::TemplateBundle>,
}
pub(crate) async fn catalog(State(s): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"targets":moleapi_generation::project::project_catalog().map_err(|_|ApiError::internal())?,"java_available":s.project_runtime.java_available(),"protoc_available":s.project_runtime.protoc.is_some(),"grpc_plugins":s.project_runtime.grpc_plugins.keys().collect::<Vec<_>>(),"native_engine":"progenitor@0.15.0","multi_language_engine":"openapi-generator@7.26.0","scope":"OpenAPI SDK/client and server project artifacts; upstream catalog does not imply per-target compile validation"}),
    ))
}
pub(crate) async fn generate(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Generate>,
) -> Result<Json<moleapi_generation::project::ProjectArtifact>, ApiError> {
    if let Some(templates) = &c.templates {
        if !moleapi_generation::project::supports_templates(&c.target) {
            return Err(ApiError::bad(
                "This generator does not expose Mustache templates",
            ));
        }
        templates
            .validate()
            .map_err(|error| ApiError::bad(error.to_string()))?;
    }
    if c.workspace_id.len() > 128 || c.specification_id.len() > 128 {
        return Err(ApiError::bad(
            "Invalid specification generation identifiers",
        ));
    }
    let template_privacy = c
        .templates
        .as_ref()
        .map(|templates| templates.privacy_value())
        .transpose()
        .map_err(|error| ApiError::bad(error.to_string()))?;
    let gate = s.protocol_admission.owner(&owner.0)?;
    let generation = *gate.lock().await;
    let w = owned(&s, &owner.0, &c.workspace_id).await?;
    let slot = s
        .project_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("Project generation capacity reached"))?;
    let lease = s.project_jobs.start(&owner.0, &w.id, &c.job_id)?;
    // Revalidate owner/workspace admission after the cancellation lease exists.
    let admission = gate.lock().await;
    if *admission != generation {
        return Err(ApiError::bad("Generation owner changed"));
    }
    owned(&s, &owner.0, &w.id).await?;
    drop(admission);
    let document = if moleapi_generation::project::is_protobuf_target(&c.target) {
        if !c.include_secrets {
            moleapi_formats::validate_protobuf_generation_options(
                &w,
                &json!({"options":c.options,"templates":template_privacy}),
            )
            .map_err(|e| ApiError::bad(e.to_string()))?;
        }
        let spec = moleapi_formats::generation_protobuf_specification(
            &w,
            &c.specification_id,
            c.include_secrets,
        )
        .map_err(|e| ApiError::bad(e.to_string()))?;
        serde_json::from_str(&spec.source).map_err(|_| ApiError::bad("Invalid protobuf source"))?
    } else {
        const PRIVACY_DOCUMENTS: &str = "x-moleapi-source-bundle-privacy-documents";
        let selected = w
            .data
            .specifications
            .iter()
            .find(|spec| spec.id == c.specification_id && spec.kind == "openapi")
            .ok_or_else(|| ApiError::bad("OpenAPI specification not found"))?;
        let bundle = moleapi_generation::project::parse_openapi_source_bundle(&selected.source)
            .map_err(|error| ApiError::bad(error.to_string()))?;
        let bundled = bundle.is_some();
        let mut projection = w.clone();
        if let Some(bundle) = bundle {
            let result = s
                .project_runtime
                .bundle_openapi(bundle, lease.cancel.clone())
                .await
                .map_err(|error| ApiError::bad(error.to_string()))?;
            let admission = gate.lock().await;
            if *admission != generation || lease.cancel.is_cancelled() {
                return Err(ApiError::bad("Generation owner changed or cancelled"));
            }
            owned(&s, &owner.0, &w.id).await?;
            drop(admission);
            let mut document = result.specification;
            if document.get(PRIVACY_DOCUMENTS).is_some() {
                return Err(ApiError::bad("Reserved source-bundle privacy field"));
            }
            document[PRIVACY_DOCUMENTS] =
                serde_json::to_value(result.documents).map_err(|_| ApiError::internal())?;
            let spec = projection
                .data
                .specifications
                .iter_mut()
                .find(|spec| spec.id == c.specification_id)
                .unwrap();
            spec.source = serde_json::to_string(&document).map_err(|_| ApiError::internal())?;
        }
        if !c.include_secrets {
            moleapi_formats::validate_generation_options(
                &projection,
                &c.specification_id,
                &json!({"options":c.options,"templates":template_privacy}),
            )
            .map_err(|e| ApiError::bad(e.to_string()))?;
        }
        let specification = moleapi_formats::generation_specification(
            &projection,
            &c.specification_id,
            c.include_secrets,
        )
        .map_err(|e| ApiError::bad(e.to_string()))?;
        if bundled {
            let mut value: Value = serde_json::from_str(&specification.source)
                .map_err(|_| ApiError::bad("Invalid bundled OpenAPI projection"))?;
            value
                .as_object_mut()
                .ok_or_else(|| ApiError::bad("Invalid bundled OpenAPI object"))?
                .remove(PRIVACY_DOCUMENTS);
            moleapi_generation::project::validate_project_specification(&value)
                .map_err(|error| ApiError::bad(error.to_string()))?;
            value
        } else {
            moleapi_generation::project::parse_project_specification(&specification.source)
                .map_err(|e| ApiError::bad(e.to_string()))?
        }
    };
    let result = s
        .project_runtime
        .generate(
            moleapi_generation::project::ProjectInput {
                specification: document,
                target: c.target,
                options: c.options,
                include_secrets: c.include_secrets,
                templates: c.templates,
            },
            lease.cancel.clone(),
        )
        .await
        .map_err(|e| ApiError::bad(e.to_string()))?;
    let admission = gate.lock().await;
    if *admission != generation || lease.cancel.is_cancelled() {
        return Err(ApiError::bad("Generation owner changed or cancelled"));
    }
    owned(&s, &owner.0, &w.id).await?;
    drop(admission);
    drop(slot);
    Ok(Json(result))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Cancel {
    job_id: String,
}
pub(crate) async fn cancel(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Cancel>,
) -> Result<Json<Value>, ApiError> {
    let entries = s.project_jobs.entries.lock().unwrap();
    let (o, _, token) = entries.get(&c.job_id).ok_or_else(ApiError::not_found)?;
    if o != &owner.0 {
        return Err(ApiError::not_found());
    }
    token.cancel();
    Ok(Json(json!({"cancelled":true})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Regenerate {
    workspace_id: String,
    job_id: String,
    previous: Vec<moleapi_generation::project::ProjectFile>,
    working: Vec<moleapi_generation::project::WorkingFile>,
    next: Vec<moleapi_generation::project::ProjectFile>,
    #[serde(default)]
    resolutions: std::collections::BTreeMap<String, String>,
}
pub(crate) async fn regenerate(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<Regenerate>,
) -> Result<Json<moleapi_generation::project::RegenerationResult>, ApiError> {
    if c.workspace_id.len() > 128 {
        return Err(ApiError::bad("Invalid regeneration workspace identifier"));
    }
    let gate = s.protocol_admission.owner(&owner.0)?;
    let generation = *gate.lock().await;
    owned(&s, &owner.0, &c.workspace_id).await?;
    let slot = s
        .project_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("Project generation capacity reached"))?;
    let lease = s.project_jobs.start(&owner.0, &c.workspace_id, &c.job_id)?;
    let admission = gate.lock().await;
    if *admission != generation {
        return Err(ApiError::bad("Regeneration owner changed"));
    }
    owned(&s, &owner.0, &c.workspace_id).await?;
    drop(admission);
    let result = s
        .project_runtime
        .regenerate(
            moleapi_generation::project::RegenerationInput {
                previous: c.previous,
                working: c.working,
                next: c.next,
                resolutions: c.resolutions,
            },
            lease.cancel.clone(),
        )
        .await
        .map_err(|e| ApiError::bad(e.to_string()))?;
    let admission = gate.lock().await;
    if *admission != generation || lease.cancel.is_cancelled() {
        return Err(ApiError::bad("Regeneration owner changed or cancelled"));
    }
    owned(&s, &owner.0, &c.workspace_id).await?;
    drop(admission);
    drop(slot);
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImportProject {
    workspace_id: String,
    job_id: String,
    role: moleapi_generation::project::ImportRole,
    source: moleapi_generation::project::ProjectImportSource,
}
pub(crate) async fn import(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Json(c): Json<ImportProject>,
) -> Result<Json<moleapi_generation::project::ImportedProject>, ApiError> {
    if c.workspace_id.len() > 128 {
        return Err(ApiError::bad("Invalid project import workspace identifier"));
    }
    let gate = s.protocol_admission.owner(&owner.0)?;
    let generation = *gate.lock().await;
    owned(&s, &owner.0, &c.workspace_id).await?;
    let slot = s
        .project_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::bad("Project generation capacity reached"))?;
    let lease = s.project_jobs.start(&owner.0, &c.workspace_id, &c.job_id)?;
    let admission = gate.lock().await;
    if *admission != generation {
        return Err(ApiError::bad("Project import owner changed"));
    }
    owned(&s, &owner.0, &c.workspace_id).await?;
    drop(admission);
    let result = s
        .project_runtime
        .import(
            moleapi_generation::project::ProjectImportInput {
                role: c.role,
                source: c.source,
            },
            lease.cancel.clone(),
        )
        .await
        .map_err(|e| ApiError::bad(e.to_string()))?;
    let admission = gate.lock().await;
    if *admission != generation || lease.cancel.is_cancelled() {
        return Err(ApiError::bad("Project import owner changed or cancelled"));
    }
    owned(&s, &owner.0, &c.workspace_id).await?;
    drop(admission);
    drop(slot);
    Ok(Json(result))
}

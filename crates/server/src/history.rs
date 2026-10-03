use crate::entities::document;
use crate::{ApiError, AppState, auth::Identity, storage, workspaces::owned};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use moleapi_core::HistoryEntry;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
pub async fn history(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<Vec<HistoryEntry>>, ApiError> {
    owned(&s, &owner.0, &id).await?;
    Ok(Json(
        storage::documents(&s.db, &owner.0, "history", Some(&id), 100).await?,
    ))
}
pub async fn clear_history(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    owned(&s, &owner.0, &id).await?;
    document::Entity::delete_many()
        .filter(document::Column::Owner.eq(owner.0))
        .filter(document::Column::Kind.eq("history"))
        .filter(document::Column::RefId.eq(id))
        .exec(&s.db)
        .await?;
    Ok(Json(serde_json::json!({"ok":true})))
}

pub(crate) async fn record(
    s: &AppState,
    owner: &str,
    w: &moleapi_core::Workspace,
    r: &moleapi_core::RequestSpec,
    environment: Option<&moleapi_core::Environment>,
    response: &moleapi_core::Response,
    privacy: crate::privacy::HistoryPrivacy<'_>,
) -> Result<(), ApiError> {
    let mut stored = response.clone();
    stored.url = moleapi_core::redact_url(&response.url, environment);
    // Logs and variable diffs are for the active execution view only.
    stored.request_updates.clear();
    stored.logs.clear();
    stored.variable_updates.clear();
    for test in &mut stored.tests {
        if test.id == "post-script-error" {
            test.actual = "[REDACTED: failed script]".into();
        }
    }
    if privacy.redact_failed_response {
        // A killed/crashed worker cannot report newly tainted values. Withhold
        // affected text instead of treating unknown privacy provenance as safe.
        stored.url = "[REDACTED: failed script]".into();
        stored.body = "[REDACTED: failed script]".into();
        stored.body_base64 = None;
        stored.headers.clear();
        for test in &mut stored.tests {
            test.name = "[REDACTED: failed script]".into();
            test.actual = "[REDACTED: failed script]".into();
            test.expected = "[REDACTED: failed script]".into();
        }
    }
    let mut secrets = privacy.values.clone();
    secrets.extend(
        [&r.auth.token, &r.auth.password]
            .into_iter()
            .filter(|v| !v.is_empty())
            .cloned(),
    );
    for header in r.headers.iter().filter(|h| {
        h.enabled
            && (h.secret == Some(true)
                || matches!(
                    h.key.to_ascii_lowercase().as_str(),
                    "authorization" | "cookie" | "x-api-key"
                ))
    }) {
        secrets.insert(header.value.clone());
    }
    for pair in r.query.iter().filter(|pair| {
        pair.enabled && (pair.secret == Some(true) || moleapi_core::sensitive_query_key(&pair.key))
    }) {
        secrets.insert(pair.value.clone());
    }
    for raw in [&r.url, &response.url] {
        if let Ok(url) = url::Url::parse(raw) {
            for (_, value) in url
                .query_pairs()
                .filter(|(key, _)| moleapi_core::sensitive_query_key(key))
            {
                secrets.insert(value.into_owned());
            }
        }
    }
    for header in r
        .headers
        .iter()
        .filter(|header| header.enabled && header.key.eq_ignore_ascii_case("authorization"))
    {
        if let Some((scheme, token)) = header.value.split_once(' ')
            && scheme.eq_ignore_ascii_case("bearer")
        {
            secrets.insert(token.trim().to_owned());
        }
    }
    let redactor = if privacy.redact_failed_response {
        // Everything affected is already withheld; compiling unknown/large
        // taint patterns cannot improve safety and would only add CPU work.
        crate::privacy::Redactor::new(&std::collections::BTreeSet::new())?
    } else {
        crate::privacy::Redactor::new(&secrets)?
    };
    // A binary response can encode secrets without containing their literal bytes.
    if !secrets.is_empty() && stored.body_base64.is_some() {
        stored.body_base64 = None;
        stored.body = "[REDACTED binary response]".into();
    }
    if r.protocol.is_soap() {
        if let Ok(decoded) = moleapi_core::soap_xml_data_values(&stored.body) {
            let mut data = serde_json::json!(decoded);
            let before = data.clone();
            redactor.scrub(&mut data);
            if data != before {
                stored.body = "[WITHHELD: private SOAP XML values]".into();
            }
        }
        stored.soap_fault = None;
        stored.body_base64 = None;
        if stored.body != "[WITHHELD: private SOAP XML values]" {
            stored.body = moleapi_core::redact_soap_xml(&stored.body)
                .unwrap_or_else(|_| "[WITHHELD: malformed/opaque SOAP XML]".into());
        }
    }
    let mut value = serde_json::to_value(&stored).map_err(|_| ApiError::internal())?;
    redactor.scrub(&mut value);
    stored = serde_json::from_value(value).map_err(|_| ApiError::internal())?;
    let entry = HistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: w.id.clone(),
        request_id: r.id.clone(),
        request_name: if privacy.redact_failed_response || redactor.withholds_text() {
            "[REDACTED: history privacy limit]".into()
        } else {
            r.name.clone()
        },
        method: r.method.clone(),
        url: stored.url.clone(),
        status: stored.status,
        elapsed_ms: stored.elapsed_ms,
        size_bytes: stored.size_bytes,
        created_at: storage::now(),
        response: stored,
    };
    let mut value = serde_json::to_value(&entry).map_err(|_| ApiError::internal())?;
    if !redactor.withholds_text() {
        redactor.scrub(&mut value);
    }
    let entry: HistoryEntry = serde_json::from_value(value).map_err(|_| ApiError::internal())?;
    // The workspace may have been deleted while the endpoint was running.
    if storage::get(&s.db, owner, &w.id).await?.is_some() {
        storage::insert_doc(&s.db, entry.id.clone(), owner, "history", &w.id, 0, &entry).await?;
    }
    Ok(())
}

use crate::{ApiError, AppState, auth::Identity, entities::document, storage, workspaces};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use futures_util::StreamExt;
use moleapi_core::Workspace;
use reqwest::{Client, Method};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone)]
struct Connection {
    server_url: String,
    username: String,
    token: String,
    identity: String,
}
#[derive(Serialize, Deserialize)]
struct Base {
    identity: String,
    local_revision: i64,
    remote_revision: i64,
    #[serde(default)]
    remote_fingerprint: Option<String>,
}
fn fingerprint(workspace: &Workspace) -> Result<String, ApiError> {
    let mut data = workspace.data.clone();
    moleapi_core::scrub_local_values(&mut data);
    let bytes = serde_json::to_vec(&(&workspace.name, &data)).map_err(|_| ApiError::internal())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
#[derive(Deserialize)]
pub struct Connect {
    server_url: String,
    username: String,
    password: String,
}
#[derive(Deserialize)]
pub struct Sync {
    resolution: Option<String>,
}
fn native(s: &AppState) -> Result<(), ApiError> {
    if s.local {
        Ok(())
    } else {
        Err(ApiError::not_found())
    }
}
async fn connection(s: &AppState) -> Result<Option<Connection>, ApiError> {
    Ok(storage::documents(&s.db, "local", "connection", None, 1)
        .await?
        .pop())
}
fn public(c: Option<&Connection>) -> serde_json::Value {
    match c {
        Some(c) => {
            serde_json::json!({"connected":true,"server_url":c.server_url,"username":c.username})
        }
        None => serde_json::json!({"connected":false}),
    }
}
pub async fn status(State(s): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    native(&s)?;
    Ok(Json(public(connection(&s).await?.as_ref())))
}
async fn client(raw: &str) -> Result<(Client, url::Url), ApiError> {
    let mut url = moleapi_core::valid_url(raw).map_err(|e| ApiError::bad(e.to_string()))?;
    if url.query().is_some() || url.fragment().is_some() || url.path() != "/" {
        return Err(ApiError::bad(
            "Sync server URL must be an origin without query or fragment",
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| ApiError::bad("Missing sync host"))?
        .trim_matches(['[', ']'])
        .to_owned();
    let port = url.port_or_known_default().unwrap_or(443);
    let ips: Vec<_> = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((host.as_str(), port)),
    )
    .await
    .map_err(|_| ApiError::bad("Sync DNS lookup timed out"))?
    .map_err(|_| ApiError::bad("Sync DNS lookup failed"))?
    .collect();
    if ips.is_empty()
        || url.scheme() != "https"
            && !(url.scheme() == "http" && ips.iter().all(|a| a.ip().is_loopback()))
    {
        return Err(ApiError::bad("Sync requires HTTPS except loopback HTTP"));
    }
    url.set_path("/");
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .resolve_to_addrs(&host, &ips)
        .build()
        .map_err(|_| ApiError::internal())?;
    Ok((client, url))
}
async fn remote(
    c: &Connection,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<Option<Workspace>, ApiError> {
    let (client, url) = client(&c.server_url).await?;
    let url = url
        .join(path)
        .map_err(|_| ApiError::bad("Invalid sync URL"))?;
    let mut req = client.request(method, url).bearer_auth(&c.token);
    if let Some(body) = body {
        req = req.json(&body);
    }
    let response = req
        .send()
        .await
        .map_err(|_| ApiError::bad("Sync server request failed"))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if status == reqwest::StatusCode::CONFLICT {
        return Err(ApiError::conflict(
            "Remote workspace changed during synchronization",
        ));
    }
    if !status.is_success() {
        return Err(ApiError::bad(format!(
            "Sync server rejected request ({})",
            status.as_u16()
        )));
    }
    let mut bytes = vec![];
    let mut stream = response.bytes_stream();
    while let Some(part) = stream.next().await {
        let part = part.map_err(|_| ApiError::bad("Reading sync response failed"))?;
        if bytes.len() + part.len() > 25 * 1024 * 1024 {
            return Err(ApiError::bad("Sync response too large"));
        }
        bytes.extend_from_slice(&part);
    }
    let w: Workspace = serde_json::from_slice(&bytes)
        .map_err(|_| ApiError::bad("Invalid sync server response"))?;
    moleapi_core::validate_workspace(&w.data).map_err(|e| ApiError::bad(e.to_string()))?;
    if !(1..i64::MAX).contains(&w.revision) || w.name.len() > 256 {
        return Err(ApiError::bad("Invalid remote workspace"));
    }
    Ok(Some(w))
}
pub async fn connect(
    State(s): State<AppState>,
    Json(c): Json<Connect>,
) -> Result<Json<serde_json::Value>, ApiError> {
    native(&s)?;
    if !(3..=64).contains(&c.username.chars().count()) || c.password.len() > 1024 {
        return Err(ApiError::bad("Invalid sync credentials"));
    }
    let _guard = s.sync_lock.lock().await;
    let (client, url) = client(&c.server_url).await?;
    let response = client
        .post(
            url.join("api/auth/login")
                .map_err(|_| ApiError::bad("Invalid sync URL"))?,
        )
        .json(&serde_json::json!({"username":c.username,"password":c.password}))
        .send()
        .await
        .map_err(|_| ApiError::bad("Sync login failed"))?;
    if !response.status().is_success() {
        return Err(ApiError::bad("Sync login rejected"));
    }
    if response.content_length().is_some_and(|n| n > 4096) {
        return Err(ApiError::bad("Invalid sync login response"));
    }
    let mut bytes = vec![];
    let mut stream = response.bytes_stream();
    while let Some(part) = stream.next().await {
        let part = part.map_err(|_| ApiError::bad("Invalid sync login response"))?;
        if bytes.len() + part.len() > 4096 {
            return Err(ApiError::bad("Invalid sync login response"));
        }
        bytes.extend_from_slice(&part);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| ApiError::bad("Invalid sync login response"))?;
    let token = value["token"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 512)
        .ok_or_else(|| ApiError::bad("Missing sync token"))?
        .to_owned();
    let previous = connection(&s).await?;
    let normalized = url.to_string();
    let same = previous
        .as_ref()
        .is_some_and(|p| p.server_url == normalized && p.username == c.username);
    let conn = Connection {
        server_url: normalized,
        username: c.username,
        token,
        identity: if same {
            previous.as_ref().unwrap().identity.clone()
        } else {
            uuid::Uuid::new_v4().to_string()
        },
    };
    let tx = s.db.begin().await?;
    document::Entity::delete_many()
        .filter(document::Column::Owner.eq("local"))
        .filter(document::Column::Kind.eq("connection"))
        .exec(&tx)
        .await?;
    if !same {
        document::Entity::delete_many()
            .filter(document::Column::Owner.eq("local"))
            .filter(document::Column::Kind.eq("base"))
            .exec(&tx)
            .await?;
    }
    storage::insert_doc(
        &tx,
        "native-connection".into(),
        "local",
        "connection",
        "",
        0,
        &conn,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(public(Some(&conn))))
}
pub async fn disconnect(State(s): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    native(&s)?;
    let _guard = s.sync_lock.lock().await;
    document::Entity::delete_many()
        .filter(document::Column::Owner.eq("local"))
        .filter(document::Column::Kind.is_in(["connection", "base"]))
        .exec(&s.db)
        .await?;
    Ok(Json(public(None)))
}
async fn save_base<C: ConnectionTrait>(db: &C, id: &str, base: &Base) -> Result<(), ApiError> {
    let key = format!("base:{}", storage::workspace_key("local", id));
    document::Entity::delete_by_id(&key).exec(db).await?;
    storage::insert_doc(db, key, "local", "base", id, 0, base).await?;
    Ok(())
}
pub async fn sync(
    State(s): State<AppState>,
    Extension(owner): Extension<Identity>,
    Path(id): Path<String>,
    Json(c): Json<Sync>,
) -> Result<Json<serde_json::Value>, ApiError> {
    native(&s)?;
    if c.resolution
        .as_deref()
        .is_some_and(|r| !matches!(r, "push" | "pull"))
    {
        return Err(ApiError::bad("Resolution must be push or pull"));
    }
    let _guard = s.sync_lock.lock().await;
    let conn = connection(&s)
        .await?
        .ok_or_else(|| ApiError::bad("No sync server connected"))?;
    let local = workspaces::owned(&s, &owner.0, &id).await?;
    const SEGMENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');
    let url_id = percent_encoding::utf8_percent_encode(&id, SEGMENT).to_string();
    let path = format!("api/workspaces/{url_id}");
    let remote_workspace = remote(&conn, Method::GET, &path, None).await?;
    if remote_workspace.as_ref().is_some_and(|r| r.id != id) {
        return Err(ApiError::bad("Remote workspace ID does not match"));
    }
    let base: Option<Base> = storage::documents(&s.db, "local", "base", Some(&id), 1)
        .await?
        .pop();
    let base = base.filter(|b| b.identity == conn.identity);
    let remote_fingerprint = remote_workspace.as_ref().map(fingerprint).transpose()?;
    let mut cloud_data = local.data.clone();
    moleapi_core::scrub_local_values(&mut cloud_data);
    let equal = remote_workspace
        .as_ref()
        .is_some_and(|r| r.name == local.name && r.data == cloud_data);
    let action = if let Some(resolution) = c.resolution.as_deref() {
        resolution
    } else if equal {
        "noop"
    } else if let Some(base) = &base {
        let lc = base.local_revision != local.revision;
        let rc = remote_workspace.as_ref().is_none_or(|r| {
            r.revision != base.remote_revision || base.remote_fingerprint != remote_fingerprint
        });
        match (lc, rc) {
            (false, false) => "noop",
            (true, false) => "push",
            (false, true) => "pull",
            (true, true) => "conflict",
        }
    } else if remote_workspace.is_none() {
        "push"
    } else {
        "conflict"
    };
    if action == "conflict" || action == "pull" && remote_workspace.is_none() {
        return Ok(Json(
            serde_json::json!({"status":"conflict","workspace":local,"remote":remote_workspace,"message":"Local and remote workspaces differ; choose push or pull"}),
        ));
    }
    let remote_final = if action == "push" {
        let (method, path, body) = match &remote_workspace {
            Some(r) => (
                Method::PUT,
                path,
                serde_json::json!({"name":local.name,"data":cloud_data,"expected_revision":r.revision}),
            ),
            None => (
                Method::POST,
                "api/workspaces".into(),
                serde_json::json!({"id":local.id,"name":local.name,"data":cloud_data}),
            ),
        };
        remote(&conn, method, &path, Some(body))
            .await?
            .ok_or_else(|| ApiError::bad("Remote workspace disappeared"))?
    } else {
        remote_workspace.ok_or_else(|| ApiError::conflict("Remote workspace disappeared"))?
    };
    if remote_final.id != id {
        return Err(ApiError::bad("Remote workspace ID does not match"));
    }
    // Fence local changes during the remote request, and commit workspace/base atomically.
    let tx = s.db.begin().await?;
    let active = document::Entity::find_by_id("native-connection")
        .one(&tx)
        .await?
        .and_then(|d| serde_json::from_str::<Connection>(&d.payload).ok());
    if active
        .as_ref()
        .is_none_or(|active| active.identity != conn.identity)
    {
        return Err(ApiError::conflict(
            "Sync connection changed during synchronization",
        ));
    }
    let current = storage::get(&tx, "local", &id)
        .await?
        .ok_or_else(|| ApiError::conflict("Local workspace deleted during sync"))?;
    if current != local {
        return Err(ApiError::conflict(
            "Local workspace changed during sync; retry",
        ));
    }
    let final_local = if action == "pull" {
        let mut next = current;
        next.name = remote_final.name.clone();
        next.data = remote_final.data.clone();
        moleapi_core::scrub_local_values(&mut next.data);
        moleapi_core::preserve_local_values(&local.data, &mut next.data);
        storage::replace_in(&tx, "local", next, local.revision)
            .await?
            .ok_or_else(|| ApiError::conflict("Local workspace changed during sync"))?
    } else {
        current
    };
    save_base(
        &tx,
        &id,
        &Base {
            identity: conn.identity,
            local_revision: final_local.revision,
            remote_revision: remote_final.revision,
            remote_fingerprint: Some(fingerprint(&remote_final)?),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        serde_json::json!({"status":"synced","workspace":final_local,"message":match action {"push"=>"Pushed local workspace","pull"=>"Pulled remote workspace",_=>"Workspaces already match"}}),
    ))
}

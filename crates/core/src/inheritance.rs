//! Source inheritance: never sign or duplicate parent credentials into saved requests.
use crate::{Auth, Collection, Environment, RequestSpec, WorkspaceData};
use anyhow::{Context, Result, ensure};
use std::collections::HashSet;
pub const MAX_COLLECTION_DEPTH: usize = 16;
pub const MAX_COLLECTIONS: usize = 1024;
/// Root-to-leaf chain; validate each parent inside this workspace and cap traversal.
pub fn collection_chain<'a>(
    data: &'a WorkspaceData,
    leaf: &'a Collection,
) -> Result<Vec<&'a Collection>> {
    ensure!(
        data.collections.len() <= MAX_COLLECTIONS,
        "Collection limit exceeded"
    );
    let mut chain = Vec::new();
    let mut current = Some(leaf);
    let mut seen = HashSet::new();
    while let Some(collection) = current {
        ensure!(
            seen.insert(collection.id.as_str()),
            "Collection hierarchy contains a cycle"
        );
        ensure!(
            chain.len() < MAX_COLLECTION_DEPTH,
            "Collection hierarchy exceeds depth limit"
        );
        chain.push(collection);
        current = collection
            .parent_id
            .as_deref()
            .map(|parent| {
                data.collections
                    .iter()
                    .find(|c| c.id == parent)
                    .context("Collection parent does not exist")
            })
            .transpose()?;
    }
    chain.reverse();
    Ok(chain)
}
#[derive(Clone, Debug, PartialEq)]
pub enum AuthenticationSource {
    Request,
    Collection(String),
    Workspace,
    Default,
}
#[derive(Clone, Debug)]
pub struct InheritedAuthentication {
    pub auth: Auth,
    pub source: AuthenticationSource,
}
/// Resolve only the mode selector. Credential/claims templates remain source until execution.
pub fn inherited_authentication(
    data: &WorkspaceData,
    collection: Option<&Collection>,
    request: &RequestSpec,
    environment: Option<&Environment>,
) -> Result<InheritedAuthentication> {
    let chain = collection
        .map(|c| collection_chain(data, c))
        .transpose()?
        .unwrap_or_default();
    let resolve = |auth: &Auth| -> Result<Option<Auth>> {
        let kind = if let Some(env) = environment {
            crate::resolve_value(&auth.kind, env)?
        } else {
            auth.kind.clone()
        };
        if kind == "inherit" {
            return Ok(None);
        }
        let mut auth = auth.clone();
        auth.kind = kind;
        crate::validate_authentication(&auth, true)?;
        Ok(Some(auth))
    };
    if let Some(auth) = resolve(&request.auth)? {
        return Ok(InheritedAuthentication {
            auth,
            source: AuthenticationSource::Request,
        });
    }
    for collection in chain.into_iter().rev() {
        if let Some(auth) = &collection.auth
            && let Some(auth) = resolve(auth)?
        {
            return Ok(InheritedAuthentication {
                auth,
                source: AuthenticationSource::Collection(collection.id.clone()),
            });
        }
    }
    if let Some(auth) = &data.auth
        && let Some(auth) = resolve(auth)?
    {
        return Ok(InheritedAuthentication {
            auth,
            source: AuthenticationSource::Workspace,
        });
    }
    Ok(InheritedAuthentication {
        auth: Auth {
            kind: "none".into(),
            token: String::new(),
            username: String::new(),
            password: String::new(),
            api_key: None,
            jwt: None,
            oauth2: None,
        },
        source: AuthenticationSource::Default,
    })
}
pub fn inherit_request_authentication(
    data: &WorkspaceData,
    collection: Option<&Collection>,
    request: &RequestSpec,
    environment: Option<&Environment>,
) -> Result<RequestSpec> {
    let mut prepared = request.clone();
    prepared.auth = inherited_authentication(data, collection, request, environment)?.auth;
    Ok(prepared)
}

/// Deterministic root-before-descendant traversal, retaining sibling source order.
pub fn collection_subtree<'a>(
    data: &'a WorkspaceData,
    root: &'a Collection,
) -> Result<Vec<&'a Collection>> {
    let mut pending = vec![root];
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    while let Some(collection) = pending.pop() {
        collection_chain(data, collection)?;
        ensure!(
            seen.insert(collection.id.as_str()),
            "Collection subtree contains duplicate IDs"
        );
        result.push(collection);
        pending.extend(
            data.collections
                .iter()
                .rev()
                .filter(|c| c.parent_id.as_deref() == Some(collection.id.as_str())),
        );
    }
    Ok(result)
}

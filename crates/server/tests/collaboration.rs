mod common;
use axum::{
    extract::{
        OriginalUri,
        ws::{Message, WebSocketUpgrade},
    },
    routing::get,
};
use common::*;
use futures_util::StreamExt;
use std::time::Duration;

#[test]
fn every_registered_route_has_an_explicit_collaboration_inventory_entry() {
    use std::collections::BTreeSet;

    let registered = include_str!("../src/lib.rs")
        .split(".route(")
        .skip(1)
        .map(|part| {
            part.trim_start()
                .strip_prefix('"')
                .and_then(|part| part.split_once('"'))
                .map(|(path, _)| path.to_owned())
                .expect("route registration must start with a literal path")
        })
        .collect::<Vec<_>>();
    let inventory = include_str!("../../../docs/COLLABORATION-ROUTE-POLICY.md")
        .lines()
        .filter_map(|line| {
            line.strip_prefix("| `")
                .and_then(|line| line.split_once('`'))
                .map(|(path, _)| path.to_owned())
        })
        .collect::<Vec<_>>();
    let registered_set = registered.iter().cloned().collect::<BTreeSet<_>>();
    let inventory_set = inventory.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        registered.len(),
        registered_set.len(),
        "duplicate route path"
    );
    assert_eq!(inventory.len(), inventory_set.len(), "duplicate policy row");
    assert_eq!(
        registered_set, inventory_set,
        "route policy inventory drift"
    );
    for path in registered_set
        .iter()
        .filter(|path| path.starts_with("/owners/"))
    {
        let row = include_str!("../../../docs/COLLABORATION-ROUTE-POLICY.md")
            .lines()
            .find(|line| line.starts_with(&format!("| `{path}` |")))
            .unwrap();
        assert!(
            !row.contains("No shared access"),
            "explicit owner/workspace route needs a member policy: {path}"
        );
    }
}

#[tokio::test]
async fn hosted_workspace_invitation_is_single_use_and_member_removal_is_immediate() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("members.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "teamowner").await;
    let member = register(&router, "teammember").await;
    let foreign = register(&router, "teamforeign").await;
    let (status, self_account) = call(&router, "GET", "/api/auth/me", Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{self_account}");
    assert_eq!(self_account["username"], "teamowner");
    assert!(self_account["id"].is_string());
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Shared","data":data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (status, invitation) = call(
        &router,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"viewer","expires_in_days":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{invitation}");
    let invite_token = invitation["token"].as_str().unwrap();
    let owner_id = invitation["owner_id"].as_str().unwrap();
    let (status, listed) = call(
        &router,
        "GET",
        "/api/workspaces/w/invitations",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert!(!listed.to_string().contains(invite_token));
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/w/invitations",
            Some(&foreign),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, accepted) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&member),
        Some(json!({"token":invite_token})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    assert_eq!(accepted["owner_id"], owner_id);
    assert_eq!(accepted["role"], "viewer");
    let (status, again) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&member),
        Some(json!({"token":invite_token})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again, accepted);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspace-invitations/accept",
            Some(&foreign),
            Some(json!({"token":invite_token}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, shared) = call(
        &router,
        "GET",
        "/api/workspaces/shared",
        Some(&member),
        None,
    )
    .await;
    assert_eq!(shared["items"][0]["workspace_id"], "w");
    assert_eq!(shared["items"][0]["role"], "viewer");
    assert_eq!(shared["items"][0]["owner_username"], "teamowner");
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/shared",
            Some(&foreign),
            None
        )
        .await
        .1,
        json!({"items":[],"next_cursor":null})
    );
    let member_id = accepted["member_id"].as_str().unwrap();
    let remove = format!("/api/owners/{owner_id}/workspaces/w/members/{member_id}");
    let members_path = format!("/api/owners/{owner_id}/workspaces/w/members");
    assert_eq!(
        call(&router, "GET", &members_path, Some(&member), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, listed_members) = call(&router, "GET", &members_path, Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{listed_members}");
    assert_eq!(listed_members.as_array().unwrap().len(), 2);
    assert_eq!(listed_members[1]["role"], "viewer");
    assert_eq!(
        call(
            &router,
            "PATCH",
            &remove,
            Some(&owner),
            Some(json!({"role":"admin","expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "PATCH",
            &remove,
            Some(&owner),
            Some(json!({"role":"editor","expected_revision":1}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, promoted) = call(
        &router,
        "GET",
        "/api/workspaces/shared",
        Some(&member),
        None,
    )
    .await;
    assert_eq!(promoted["items"][0]["role"], "admin");
    let audit_path = format!("/api/owners/{owner_id}/workspaces/w/audit");
    let (status, audit) = call(&router, "GET", &audit_path, Some(&member), None).await;
    assert_eq!(status, StatusCode::OK, "{audit}");
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|event| { event["action"] == "membership.role" && event["actor_id"] == owner_id })
    );
    assert_eq!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["action"] == "membership.role")
            .count(),
        1,
        "a rejected role CAS must not produce an audit success"
    );
    assert_eq!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["action"] == "membership.accept")
            .count(),
        1,
        "idempotent invitation retry must not duplicate the audit event"
    );
    let scoped_invites = format!("/api/owners/{owner_id}/workspaces/w/invitations");
    let (status, second_invite) = call(
        &router,
        "POST",
        &scoped_invites,
        Some(&member),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second_invite}");
    assert!(
        !call(&router, "GET", &scoped_invites, Some(&member), None)
            .await
            .1
            .to_string()
            .contains(second_invite["token"].as_str().unwrap())
    );
    let (status, second_member) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&foreign),
        Some(json!({"token":second_invite["token"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second_member}");
    assert_eq!(second_member["role"], "editor");
    let second_remove = format!(
        "{members_path}/{}",
        second_member["member_id"].as_str().unwrap()
    );
    assert_eq!(
        call(&router, "DELETE", &second_remove, Some(&member), None)
            .await
            .0,
        StatusCode::OK
    );
    let (status, pending) = call(
        &router,
        "POST",
        &scoped_invites,
        Some(&owner),
        Some(json!({"role":"viewer","expires_in_days":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pending}");
    assert_eq!(
        call(
            &router,
            "DELETE",
            &format!("{scoped_invites}/{}", pending["id"].as_str().unwrap()),
            Some(&owner),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspace-invitations/accept",
            Some(&foreign),
            Some(json!({"token":pending["token"]}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "DELETE", &remove, Some(&foreign), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "DELETE", &remove, Some(&owner), None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/shared",
            Some(&member),
            None
        )
        .await
        .1,
        json!({"items":[],"next_cursor":null})
    );
}

#[tokio::test]
async fn deleting_and_recreating_workspace_never_restores_old_membership_or_invites() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("delete-members.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "deleteowner").await;
    let member = register(&router, "deletemember").await;
    let (_, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Before","data":data()})),
    )
    .await;
    let (_, accepted_invite) = call(
        &router,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"viewer","expires_in_days":1})),
    )
    .await;
    let (_, pending_invite) = call(
        &router,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspace-invitations/accept",
            Some(&member),
            Some(json!({"token":accepted_invite["token"]}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"expected_revision":workspace["revision"]}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/shared",
            Some(&member),
            None
        )
        .await
        .1,
        json!({"items":[],"next_cursor":null})
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspace-invitations/accept",
            Some(&member),
            Some(json!({"token":pending_invite["token"]}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, recreated) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"After","data":data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{recreated}");
    assert!(recreated["revision"].as_i64().unwrap() > workspace["revision"].as_i64().unwrap());
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/shared",
            Some(&member),
            None
        )
        .await
        .1,
        json!({"items":[],"next_cursor":null})
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/w/invitations",
            Some(&owner),
            None
        )
        .await
        .1,
        json!([])
    );
    let audit_path = format!(
        "/api/owners/{}/workspaces/w/audit",
        accepted_invite["owner_id"].as_str().unwrap()
    );
    assert_eq!(
        call(&router, "GET", &audit_path, Some(&owner), None)
            .await
            .1,
        json!([])
    );
}

#[tokio::test]
async fn concurrent_invitation_acceptance_has_one_winner_across_instances() {
    let dir = tempfile::tempdir().unwrap();
    let url = moleapi_server::sqlite_database_url(&dir.path().join("invite-race.db")).unwrap();
    let first = hosted(config(url.clone(), true)).await.unwrap();
    let second = hosted(config(url, true)).await.unwrap();
    let owner = register(&first, "raceowner").await;
    let alice = register(&first, "racealice").await;
    let bob = register(&first, "racebobby").await;
    assert_eq!(
        call(
            &first,
            "POST",
            "/api/workspaces",
            Some(&owner),
            Some(json!({"id":"w","name":"Race","data":data()}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, invite) = call(
        &first,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"viewer","expires_in_days":1})),
    )
    .await;
    let body = json!({"token":invite["token"]});
    let (a, b) = tokio::join!(
        call(
            &first,
            "POST",
            "/api/workspace-invitations/accept",
            Some(&alice),
            Some(body.clone())
        ),
        call(
            &second,
            "POST",
            "/api/workspace-invitations/accept",
            Some(&bob),
            Some(body)
        )
    );
    assert!(
        (a.0 == StatusCode::OK && b.0 == StatusCode::NOT_FOUND)
            || (b.0 == StatusCode::OK && a.0 == StatusCode::NOT_FOUND),
        "{a:?} {b:?}"
    );
    let alice_shared = call(&first, "GET", "/api/workspaces/shared", Some(&alice), None)
        .await
        .1;
    let bob_shared = call(&second, "GET", "/api/workspaces/shared", Some(&bob), None)
        .await
        .1;
    assert_eq!(
        alice_shared["items"].as_array().unwrap().len()
            + bob_shared["items"].as_array().unwrap().len(),
        1
    );
}

#[tokio::test]
async fn viewer_gets_safe_projection_and_editor_writes_canonical_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("shared-access.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "accessowner").await;
    let member = register(&router, "accessmember").await;
    let mut source = example_data();
    source["global_variables"] =
        json!([{"id":"s","key":"secret","value":"private-sentinel","enabled":true,"secret":true}]);
    source["collections"][0]["variables"] = json!([
        {"id":"collection-private","key":"collection_token","value":"private-collection-sentinel","enabled":true,"secret":true}
    ]);
    source["environments"] = json!([{"id":"team-dev","name":"Team dev","variables":[
        {"id":"environment-private","key":"environment_token","value":"private-environment-sentinel","enabled":true,"secret":true}
    ]}]);
    source["active_environment_id"] = json!("team-dev");
    source["documents"] = json!([
        {"id":"private-doc","name":"Private guide","source":"private-document-sentinel","secret":true},
        {"id":"public-doc","name":"Public guide","source":"# Public guide","secret":false}
    ]);
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Team","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{workspace}");
    let (_, invite) = call(
        &router,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"viewer","expires_in_days":1})),
    )
    .await;
    let (_, accepted) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&member),
        Some(json!({"token":invite["token"]})),
    )
    .await;
    let owner_id = accepted["owner_id"].as_str().unwrap();
    let member_id = accepted["member_id"].as_str().unwrap();
    let shared_path = format!("/api/owners/{owner_id}/workspaces/w");
    let audit_path = format!("{shared_path}/audit");
    let (status, viewer) = call(&router, "GET", &shared_path, Some(&member), None).await;
    assert_eq!(status, StatusCode::OK, "{viewer}");
    assert!(!viewer.to_string().contains("private-sentinel"));
    assert!(!viewer.to_string().contains("private-collection-sentinel"));
    assert!(!viewer.to_string().contains("private-environment-sentinel"));
    assert!(!viewer.to_string().contains("private-document-sentinel"));
    assert!(viewer.to_string().contains("# Public guide"));
    assert_eq!(
        call(&router, "GET", &audit_path, Some(&member), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "PUT",
            &shared_path,
            Some(&member),
            Some(json!({"name":"Denied","expected_revision":1,"data":workspace["data"]}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let role_path = format!("{shared_path}/members/{member_id}");
    assert_eq!(
        call(
            &router,
            "PATCH",
            &role_path,
            Some(&owner),
            Some(json!({"role":"editor","expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, editor) = call(&router, "GET", &shared_path, Some(&member), None).await;
    assert_eq!(status, StatusCode::OK, "{editor}");
    assert!(editor.to_string().contains("private-sentinel"));
    assert!(editor.to_string().contains("private-document-sentinel"));
    let mut edited_data = editor["data"].clone();
    edited_data["collections"][0]["requests"][0]["examples"][0]["body"] = json!("edited by member");
    edited_data["documents"][0]["source"] = json!("# Edited member document");
    edited_data["global_variables"][0]["value"] = json!("shared-global");
    edited_data["global_variables"][0]["local_value"] = json!("member-local-sentinel");
    edited_data["collections"][0]["variables"][0]["value"] = json!("shared-collection");
    edited_data["environments"][0]["name"] = json!("Renamed team dev");
    edited_data["environments"][0]["variables"][0]["value"] = json!("shared-environment");
    let mut invalid_data = edited_data.clone();
    invalid_data["global_variables"][0]["key"] = json!("");
    assert_eq!(
        call(
            &router,
            "PUT",
            &shared_path,
            Some(&member),
            Some(
                json!({"name":"Invalid","expected_revision":editor["revision"],"data":invalid_data})
            )
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, updated) = call(
        &router,
        "PUT",
        &shared_path,
        Some(&member),
        Some(json!({"name":"Edited","expected_revision":editor["revision"],"data":edited_data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        call(
            &router,
            "PUT",
            &shared_path,
            Some(&member),
            Some(json!({"name":"Stale","expected_revision":editor["revision"],"data":edited_data}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, preview) = call(
        &router,
        "GET",
        &format!("{shared_path}/mock/r/e"),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{preview}");
    assert_eq!(preview["raw"], "edited by member");
    let (status, owner_preview) = call(&router, "GET", "/api/mock/w/r/e", Some(&owner), None).await;
    assert_eq!(status, StatusCode::CREATED, "{owner_preview}");
    assert_eq!(owner_preview["raw"], "edited by member");
    let (_, owner_workspace) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(
        owner_workspace["data"]["documents"][0]["source"],
        "# Edited member document"
    );
    assert_eq!(owner_workspace["revision"], updated["revision"]);
    assert_eq!(
        owner_workspace["data"]["global_variables"][0]["value"],
        "shared-global"
    );
    assert!(owner_workspace["data"]["global_variables"][0]["local_value"].is_null());
    assert_eq!(
        owner_workspace["data"]["collections"][0]["variables"][0]["value"],
        "shared-collection"
    );
    assert_eq!(
        owner_workspace["data"]["environments"][0]["name"],
        "Renamed team dev"
    );
    assert_eq!(
        owner_workspace["data"]["environments"][0]["variables"][0]["value"],
        "shared-environment"
    );
    let (status, audit) = call(&router, "GET", &audit_path, Some(&owner), None).await;
    assert_eq!(status, StatusCode::OK, "{audit}");
    assert!(
        audit.as_array().unwrap().iter().any(|event| {
            event["action"] == "workspace.update" && event["actor_id"] == member_id
        })
    );
    assert!(!audit.to_string().contains("private-sentinel"));
    assert!(
        !audit
            .to_string()
            .contains(invite["token"].as_str().unwrap())
    );
    assert_eq!(
        call(&router, "GET", "/api/workspaces/w", Some(&owner), None)
            .await
            .1["name"],
        "Edited"
    );
    assert_eq!(
        call(&router, "DELETE", &role_path, Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "GET", &shared_path, Some(&member), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, audit_after_removal) = call(&router, "GET", &audit_path, Some(&owner), None).await;
    assert!(
        audit_after_removal.as_array().unwrap().iter().any(|event| {
            event["action"] == "membership.remove" && event["actor_id"] == owner_id
        })
    );
}

#[tokio::test]
async fn shared_workspace_ids_remain_scoped_to_their_owner() {
    let dir = tempfile::tempdir().unwrap();
    let router = hosted(config(
        moleapi_server::sqlite_database_url(&dir.path().join("owner-scope.db")).unwrap(),
        true,
    ))
    .await
    .unwrap();
    let owner_a = register(&router, "scopeownera").await;
    let owner_b = register(&router, "scopeownerb").await;
    let member = register(&router, "scopemember").await;
    for (owner, name) in [(&owner_a, "Alpha"), (&owner_b, "Beta")] {
        let (status, workspace) = call(
            &router,
            "POST",
            "/api/workspaces",
            Some(owner),
            Some(json!({"id":"same","name":name,"data":data()})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{workspace}");
    }
    let (_, invitation) = call(
        &router,
        "POST",
        "/api/workspaces/same/invitations",
        Some(&owner_a),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    let owner_a_id = invitation["owner_id"].as_str().unwrap();
    let (_, accepted) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&member),
        Some(json!({"token":invitation["token"]})),
    )
    .await;
    assert_eq!(accepted["owner_id"], owner_a_id);
    let (_, shared) = call(
        &router,
        "GET",
        "/api/workspaces/shared",
        Some(&member),
        None,
    )
    .await;
    assert_eq!(shared["items"].as_array().unwrap().len(), 1);
    assert_eq!(shared["items"][0]["owner_id"], owner_a_id);
    let path_a = format!("/api/owners/{owner_a_id}/workspaces/same");
    let (_, visible) = call(&router, "GET", &path_a, Some(&member), None).await;
    assert_eq!(visible["name"], "Alpha");
    let (_, invitation_b) = call(
        &router,
        "POST",
        "/api/workspaces/same/invitations",
        Some(&owner_b),
        Some(json!({"role":"viewer","expires_in_days":1})),
    )
    .await;
    let owner_b_id = invitation_b["owner_id"].as_str().unwrap();
    let path_b = format!("/api/owners/{owner_b_id}/workspaces/same");
    assert_eq!(
        call(&router, "GET", &path_b, Some(&member), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&router, "GET", "/api/workspaces/same", Some(&member), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, updated) = call(
        &router,
        "PUT",
        &path_a,
        Some(&member),
        Some(json!({"name":"Edited Alpha","expected_revision":visible["revision"],"data":visible["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        call(&router, "GET", "/api/workspaces/same", Some(&owner_b), None)
            .await
            .1["name"],
        "Beta"
    );
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&member),
        Some(json!({"token":invitation_b["token"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, first_page) = call(
        &router,
        "GET",
        "/api/workspaces/shared?limit=1",
        Some(&member),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first_page}");
    assert_eq!(first_page["items"].as_array().unwrap().len(), 1);
    let cursor = first_page["next_cursor"].as_str().unwrap();
    let (status, second_page) = call(
        &router,
        "GET",
        &format!("/api/workspaces/shared?limit=1&cursor={cursor}"),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second_page}");
    assert_eq!(second_page["items"].as_array().unwrap().len(), 1);
    assert!(second_page["next_cursor"].is_null());
    assert_ne!(
        first_page["items"][0]["owner_id"],
        second_page["items"][0]["owner_id"]
    );
}

#[tokio::test]
async fn editor_run_is_audited_and_cross_instance_removal_withholds_its_result() {
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("shared-run.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let first = hosted(settings.clone()).await.unwrap();
    let second = hosted(settings).await.unwrap();
    let owner = register(&first, "runowner").await;
    let member = register(&first, "runmember").await;
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (url, server) = serve(
        Router::new()
            .route("/fast", axum::routing::get(|| async { "fast" }))
            .route(
                "/slow",
                axum::routing::get({
                    let started = started.clone();
                    let release = release.clone();
                    move || {
                        let started = started.clone();
                        let release = release.clone();
                        async move {
                            started.notify_one();
                            release.notified().await;
                            "slow-private-response"
                        }
                    }
                }),
            ),
    )
    .await;
    let mut workspace_data = example_data();
    let mut fast = workspace_data["collections"][0]["requests"][0].clone();
    fast["id"] = json!("fast");
    fast["url"] = json!(format!("{url}/fast"));
    fast["examples"][0]["delay_ms"] = json!(450);
    let mut unauthorized_example = fast["examples"][0].clone();
    unauthorized_example["id"] = json!("auth-example");
    unauthorized_example["status"] = json!(401);
    unauthorized_example["body"] = json!("intentional Mock 401");
    fast["examples"]
        .as_array_mut()
        .unwrap()
        .push(unauthorized_example);
    let mut slow = fast.clone();
    slow["id"] = json!("slow");
    slow["url"] = json!(format!("{url}/slow"));
    slow["timeout_ms"] = json!(10000);
    workspace_data["collections"][0]["requests"] = json!([fast, slow]);
    let (status, created) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Shared runner","data":workspace_data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let (_, invitation) = call(
        &first,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"viewer","expires_in_days":1})),
    )
    .await;
    let (_, accepted) = call(
        &first,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&member),
        Some(json!({"token":invitation["token"]})),
    )
    .await;
    let owner_id = accepted["owner_id"].as_str().unwrap();
    let member_id = accepted["member_id"].as_str().unwrap();
    let shared_path = format!("/api/owners/{owner_id}/workspaces/w");
    let run_path = format!("{shared_path}/run");
    let execute_path = format!("{shared_path}/requests/fast/execute");
    let mock_path = format!("{shared_path}/mock/fast/e");
    let mock_calls_path = format!("{shared_path}/mock-calls");
    let history_path = format!("{shared_path}/history");
    let reports_path = format!("{shared_path}/reports");
    assert_eq!(
        call(
            &first,
            "POST",
            &run_path,
            Some(&member),
            Some(json!({"collection_id":"c","request_ids":["fast"]}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &execute_path,
            Some(&member),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&first, "GET", &mock_path, Some(&member), None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&first, "GET", &mock_calls_path, Some(&member), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    for path in [&history_path, &reports_path] {
        assert_eq!(
            call(&first, "GET", path, Some(&member), None).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let role_path = format!("{shared_path}/members/{member_id}");
    assert_eq!(
        call(
            &second,
            "PATCH",
            &role_path,
            Some(&owner),
            Some(json!({"role":"editor","expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, report) = call(
        &first,
        "POST",
        &run_path,
        Some(&member),
        Some(json!({"collection_id":"c","request_ids":["fast"],"job_id":"fast-job"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["results"][0]["response"]["status"], 200);
    let report_id = report["report_id"].as_str().unwrap();
    let (status, history) = call(&first, "GET", &history_path, Some(&member), None).await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert_eq!(history.as_array().unwrap().len(), 1);
    let (status, summaries) = call(&first, "GET", &reports_path, Some(&member), None).await;
    assert_eq!(status, StatusCode::OK, "{summaries}");
    assert_eq!(summaries["items"][0]["id"], report_id);
    let detail_path = format!("{reports_path}/{report_id}");
    let (status, detail) = call(&first, "GET", &detail_path, Some(&member), None).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["id"], report_id);
    let position = detail["results"][0]["position"].as_u64().unwrap();
    let response_path = format!("{detail_path}/steps/{position}/response");
    let (status, saved_response) = call(&first, "GET", &response_path, Some(&member), None).await;
    assert_eq!(status, StatusCode::OK, "{saved_response}");
    assert_eq!(saved_response["response"]["status"], 200);
    let (status, direct) = call(
        &first,
        "POST",
        &execute_path,
        Some(&member),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{direct}");
    assert_eq!(direct["status"], 200);
    let (status, mock_response) = call(&first, "GET", &mock_path, Some(&member), None).await;
    assert_eq!(status, StatusCode::CREATED, "{mock_response}");
    assert_eq!(mock_response["raw"], "example body");
    let auth_example_response = first
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("{shared_path}/mock/fast/auth-example"))
                .header("authorization", format!("Bearer {member}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(auth_example_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        auth_example_response.headers()["x-moleapi-mock-preview"],
        "saved-example"
    );
    let (status, mock_calls) = call(&first, "GET", &mock_calls_path, Some(&member), None).await;
    assert_eq!(status, StatusCode::OK, "{mock_calls}");
    assert!(
        mock_calls
            .as_array()
            .unwrap()
            .iter()
            .any(|call| { call["request_id"] == "fast" && call["example_id"] == "e" })
    );
    let first_for_mock = first.clone();
    let member_for_mock = member.clone();
    let mock_path_for_task = mock_path.clone();
    let delayed_mock = tokio::spawn(async move {
        call(
            &first_for_mock,
            "GET",
            &mock_path_for_task,
            Some(&member_for_mock),
            None,
        )
        .await
    });
    let audit_path = format!("{shared_path}/audit");
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let (_, events) = call(&second, "GET", &audit_path, Some(&owner), None).await;
            if events
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["action"] == "mock.preview")
                .count()
                >= 2
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        call(
            &second,
            "PATCH",
            &role_path,
            Some(&owner),
            Some(json!({"role":"viewer","expected_revision":2}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, withheld_mock) =
        tokio::time::timeout(std::time::Duration::from_secs(3), delayed_mock)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(status, StatusCode::NOT_FOUND, "{withheld_mock}");
    assert!(!withheld_mock.to_string().contains("example body"));
    assert_eq!(
        call(
            &second,
            "PATCH",
            &role_path,
            Some(&owner),
            Some(json!({"role":"editor","expected_revision":3}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &execute_path,
            Some(&member),
            Some(json!({"request":{"url":"http://attacker.invalid"}}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &format!("{shared_path}/requests/missing/execute"),
            Some(&member),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let first_for_run = first.clone();
    let member_for_run = member.clone();
    let run_path_for_task = run_path.clone();
    let slow_run = tokio::spawn(async move {
        call(
            &first_for_run,
            "POST",
            &run_path_for_task,
            Some(&member_for_run),
            Some(json!({"collection_id":"c","request_ids":["slow"],"job_id":"slow-job"})),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    assert_eq!(
        call(&second, "DELETE", &role_path, Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
    let (status, withheld) = tokio::time::timeout(std::time::Duration::from_secs(5), slow_run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::NOT_FOUND, "{withheld}");
    assert!(!withheld.to_string().contains("slow-private-response"));
    assert_eq!(
        call(
            &first,
            "POST",
            &execute_path,
            Some(&member),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for path in [&mock_path, &mock_calls_path] {
        assert_eq!(
            call(&first, "GET", path, Some(&member), None).await.0,
            StatusCode::NOT_FOUND
        );
    }
    for path in [&history_path, &reports_path, &detail_path, &response_path] {
        assert_eq!(
            call(&first, "GET", path, Some(&member), None).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let (_, audit) = call(
        &second,
        "GET",
        &format!("{shared_path}/audit"),
        Some(&owner),
        None,
    )
    .await;
    assert!(audit.as_array().unwrap().iter().any(|event| {
        event["actor_id"] == member_id
            && event["action"] == "workspace.run"
            && event["subject_id"] == "slow-job"
            && event["result"] == "started"
    }));
    assert!(audit.as_array().unwrap().iter().any(|event| {
        event["actor_id"] == member_id
            && event["action"] == "request.execute"
            && event["subject_id"] == "fast"
    }));
    assert!(audit.as_array().unwrap().iter().any(|event| {
        event["actor_id"] == member_id
            && event["action"] == "mock.preview"
            && event["subject_id"] == "fast"
    }));
    release.notify_one();
    server.abort();
}

#[tokio::test]
async fn shared_websocket_session_binds_actor_and_closes_after_cross_instance_role_change() {
    let (seen, mut received_urls) = tokio::sync::mpsc::channel(4);
    let (closed, mut peer_closed) = tokio::sync::mpsc::channel(4);
    let fixture = Router::new().route(
        "/socket",
        get(
            move |upgrade: WebSocketUpgrade, OriginalUri(uri): OriginalUri| {
                let seen = seen.clone();
                let closed = closed.clone();
                async move {
                    let _ = seen.send(uri.to_string()).await;
                    upgrade.on_upgrade(move |mut socket| async move {
                        while let Some(Ok(message)) = socket.recv().await {
                            if matches!(message, Message::Close(_)) {
                                break;
                            }
                            if socket.send(message).await.is_err() {
                                break;
                            }
                        }
                        let _ = closed.send(()).await;
                    })
                }
            },
        ),
    );
    let (url, server) = serve(fixture).await;
    let ws_url = url.replacen("http", "ws", 1);
    let dir = tempfile::tempdir().unwrap();
    let db = moleapi_server::sqlite_database_url(&dir.path().join("shared-live.db")).unwrap();
    let mut options = config(db.clone(), true);
    options.allow_private_network = true;
    let first = hosted(options.clone()).await.unwrap();
    let second = hosted(options).await.unwrap();
    let owner = register(&first, "liveowner").await;
    let editor = register(&first, "liveeditor").await;
    let other = register(&first, "liveother").await;
    let viewer = register(&first, "liveviewer").await;
    let mut source = example_data();
    source["collections"][0]["requests"][0]["url"] =
        json!(format!("{ws_url}/socket?region={{{{region}}}}"));
    source["collections"][0]["requests"][0]["protocol"] = json!({"kind":"websocket"});
    source["global_variables"] =
        json!([{"id":"region","key":"region","value":"team","enabled":true}]);
    let (status, created) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Live","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let owner_id = call(&first, "GET", "/api/auth/me", Some(&owner), None)
        .await
        .1["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let base = format!("/api/owners/{owner_id}/workspaces/w");
    let mut member_ids = Vec::new();
    for (token, role) in [(&editor, "editor"), (&other, "editor"), (&viewer, "viewer")] {
        let (status, invitation) = call(
            &first,
            "POST",
            "/api/workspaces/w/invitations",
            Some(&owner),
            Some(json!({"role":role,"expires_in_days":1})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{invitation}");
        let (status, accepted) = call(
            &second,
            "POST",
            "/api/workspace-invitations/accept",
            Some(token),
            Some(json!({"token":invitation["token"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{accepted}");
        member_ids.push(accepted["member_id"].as_str().unwrap().to_owned());
    }
    let create_path = format!("{base}/sessions");
    let input = json!({"request_id":"r","expected_revision":1});
    assert_eq!(
        call(
            &first,
            "POST",
            &create_path,
            Some(&viewer),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &create_path,
            Some(&editor),
            Some(json!({"request_id":"r","expected_revision":0}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &create_path,
            Some(&editor),
            Some(json!({"request_id":"missing","expected_revision":1}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, opened) = call(&first, "POST", &create_path, Some(&editor), Some(input)).await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let id = opened["id"].as_str().unwrap();
    let path = format!("{create_path}/{id}");
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), received_urls.recv())
            .await
            .unwrap()
            .unwrap(),
        "/socket?region=team"
    );
    assert_eq!(
        call(
            &first,
            "GET",
            &format!("/api/sessions/{id}"),
            Some(&editor),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&first, "GET", &path, Some(&other), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&first, "GET", &path, Some(&viewer), None).await.0,
        StatusCode::NOT_FOUND
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (_, summary) = call(&first, "GET", &path, Some(&editor), None).await;
            if summary["state"] == "open" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        call(
            &first,
            "POST",
            &format!("{path}/send"),
            Some(&editor),
            Some(json!({"kind":"text","text":"shared echo"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (status, batch) = call(
                &first,
                "GET",
                &format!("{path}/events?after=0"),
                Some(&editor),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{batch}");
            if batch["events"].as_array().unwrap().iter().any(|event| {
                event["direction"] == "incoming" && event["message"]["text"] == "shared echo"
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let audit = call(&first, "GET", &format!("{base}/audit"), Some(&owner), None)
        .await
        .1;
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["action"] == "protocol.session.start"
                && event["actor_id"] == member_ids[0])
    );
    assert!(!audit.to_string().contains("shared echo"));
    assert_eq!(
        call(
            &second,
            "PATCH",
            &format!("{base}/members/{}", member_ids[0]),
            Some(&owner),
            Some(json!({"role":"viewer","expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if call(&first, "GET", &path, Some(&editor), None).await.0 == StatusCode::NOT_FOUND {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), peer_closed.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        call(
            &first,
            "GET",
            &format!("{path}/events"),
            Some(&editor),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    server.abort();
}

#[tokio::test]
async fn shared_sse_session_withholds_private_events_after_workspace_revision_and_logout() {
    let fixture = Router::new().route(
        "/events",
        get(|| async {
            (
                [("content-type", "text/event-stream")],
                Body::from_stream(
                    futures_util::stream::once(async {
                        Ok::<_, std::io::Error>("data: private-live-sentinel\n\n")
                    })
                    .chain(futures_util::stream::pending()),
                ),
            )
        }),
    );
    let (url, server) = serve(fixture).await;
    let dir = tempfile::tempdir().unwrap();
    let db = moleapi_server::sqlite_database_url(&dir.path().join("shared-sse.db")).unwrap();
    let mut options = config(db.clone(), true);
    options.allow_private_network = true;
    let first = hosted(options.clone()).await.unwrap();
    let second = hosted(options).await.unwrap();
    let owner = register(&first, "sseowner").await;
    let member = register(&first, "ssemember").await;
    let mut source = example_data();
    source["collections"][0]["requests"][0]["url"] = json!(format!("{url}/events"));
    source["collections"][0]["requests"][0]["protocol"] = json!({"kind":"sse"});
    source["global_variables"] = json!([{"id":"private","key":"api_key","value":"private-live-sentinel","enabled":true,"secret":true}]);
    let (status, canonical) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Live","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{canonical}");
    let (_, invite) = call(
        &first,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    let (_, accepted) = call(
        &second,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&member),
        Some(json!({"token":invite["token"]})),
    )
    .await;
    let base = format!(
        "/api/owners/{}/workspaces/w/sessions",
        accepted["owner_id"].as_str().unwrap()
    );
    let (status, opened) = call(
        &first,
        "POST",
        &base,
        Some(&member),
        Some(json!({"request_id":"r","expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let path = format!("{base}/{}", opened["id"].as_str().unwrap());
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (status, batch) = call(
                &first,
                "GET",
                &format!("{path}/events"),
                Some(&member),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{batch}");
            if batch["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["message"]["kind"] == "sse")
            {
                assert!(batch.to_string().contains("private-live-sentinel"));
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        !call(&first, "GET", &path, Some(&member), None)
            .await
            .1
            .to_string()
            .contains("private-live-sentinel")
    );
    assert_eq!(
        call(
            &first,
            "POST",
            &format!("{path}/send"),
            Some(&member),
            Some(json!({"kind":"text","text":"denied"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, updated) = call(
        &second,
        "PUT",
        "/api/workspaces/w",
        Some(&owner),
        Some(json!({"name":"Changed","data":canonical["data"],"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if call(&first, "GET", &path, Some(&member), None).await.0 == StatusCode::NOT_FOUND {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        call(
            &first,
            "GET",
            &format!("{path}/events"),
            Some(&member),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, opened) = call(
        &first,
        "POST",
        &base,
        Some(&member),
        Some(json!({"request_id":"r","expected_revision":2})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    let path = format!("{base}/{}", opened["id"].as_str().unwrap());
    assert_eq!(
        call(&second, "POST", "/api/auth/logout", Some(&member), None)
            .await
            .0,
        StatusCode::OK
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if call(&first, "GET", &path, Some(&member), None).await.0 == StatusCode::UNAUTHORIZED {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    server.abort();
}

#[tokio::test]
async fn editor_http_settings_save_to_owner_and_execute_on_the_wire() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("shared-http.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "httpowner").await;
    let member = register(&router, "httpmember").await;
    let (url, fixture) = serve(Router::new().route(
        "/echo",
        axum::routing::post(
            |uri: axum::http::Uri, headers: axum::http::HeaderMap, body: String| async move {
                axum::Json(json!({
                    "query":uri.query(),
                    "team_header":headers.get("x-team").and_then(|value| value.to_str().ok()),
                    "auth_valid":headers.get("authorization").and_then(|value| value.to_str().ok()) == Some("Bearer team-secret"),
                    "body":body,
                }))
            },
        ),
    ))
    .await;
    let (status, _) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Team HTTP","data":example_data()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, invitation) = call(
        &router,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    let (_, accepted) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&member),
        Some(json!({"token":invitation["token"]})),
    )
    .await;
    let owner_id = accepted["owner_id"].as_str().unwrap();
    let member_id = accepted["member_id"].as_str().unwrap();
    let shared = format!("/api/owners/{owner_id}/workspaces/w");
    let (_, current) = call(&router, "GET", &shared, Some(&member), None).await;
    let mut data = current["data"].clone();
    let request = &mut data["collections"][0]["requests"][0];
    request["method"] = json!("POST");
    request["url"] = json!(format!("{url}/echo"));
    request["query"] = json!([{"id":"query","key":"source","value":"member","enabled":true}]);
    request["headers"] = json!([{"id":"header","key":"X-Team","value":"shared","enabled":true}]);
    request["body_kind"] = json!("json");
    request["body"] = json!(r#"{"hello":"team"}"#);
    request["auth"] = json!({"kind":"bearer","token":"team-secret","username":"","password":""});
    let (status, updated) = call(
        &router,
        "PUT",
        &shared,
        Some(&member),
        Some(json!({"name":"Team HTTP","expected_revision":current["revision"],"data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let (_, owner_workspace) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(owner_workspace["revision"], updated["revision"]);
    let (status, response) = call(
        &router,
        "POST",
        &format!("{shared}/requests/r/execute"),
        Some(&member),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], 200);
    let body: serde_json::Value = serde_json::from_str(response["body"].as_str().unwrap()).unwrap();
    assert_eq!(body["query"], "source=member");
    assert_eq!(body["team_header"], "shared");
    assert_eq!(body["auth_valid"], true);
    assert_eq!(body["body"], r#"{"hello":"team"}"#);
    let (status, history) = call(
        &router,
        "GET",
        &format!("{shared}/history"),
        Some(&member),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert_eq!(history[0]["status"], 200);
    let (status, audit) = call(
        &router,
        "GET",
        &format!("{shared}/audit"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{audit}");
    assert!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["action"] == "request.execute" && event["actor_id"] == member_id)
    );
    fixture.abort();
}

#[tokio::test]
async fn editor_draft_http_executes_without_saving_and_rejects_invalid_admission() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("shared-draft.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "draftowner").await;
    let editor = register(&router, "drafteditor").await;
    let viewer = register(&router, "draftviewer").await;
    let foreign = register(&router, "draftforeign").await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (url, fixture) = serve(Router::new().route(
        "/echo",
        axum::routing::post({
            let calls = calls.clone();
            move |uri: axum::http::Uri, headers: axum::http::HeaderMap, body: String| {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    axum::Json(json!({"query":uri.query(),"body":body,"parent_auth":headers.get("authorization").and_then(|value| value.to_str().ok()) == Some("Bearer parent-token")}))
                }
            }
        }),
    ))
    .await;
    let mut source = example_data();
    source["collections"][0]["variables"] =
        json!([{"id":"parent-var","key":"parent_value","value":"from-parent","enabled":true}]);
    source["collections"][0]["auth"] =
        json!({"kind":"bearer","token":"parent-token","username":"","password":""});
    let mut other = source["collections"][0].clone();
    other["id"] = json!("other-collection");
    other["requests"][0]["id"] = json!("other-request");
    source["collections"].as_array_mut().unwrap().push(other);
    let (status, created) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Draft workspace","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&foreign),
            Some(json!({"id":"w","name":"Foreign workspace","data":example_data()})),
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut invitations = Vec::new();
    for role in ["editor", "viewer"] {
        invitations.push(
            call(
                &router,
                "POST",
                "/api/workspaces/w/invitations",
                Some(&owner),
                Some(json!({"role":role,"expires_in_days":1})),
            )
            .await
            .1,
        );
    }
    let (_, accepted) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&editor),
        Some(json!({"token":invitations[0]["token"]})),
    )
    .await;
    let (_, _) = call(
        &router,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&viewer),
        Some(json!({"token":invitations[1]["token"]})),
    )
    .await;
    let owner_id = accepted["owner_id"].as_str().unwrap();
    let editor_id = accepted["member_id"].as_str().unwrap();
    let draft_path = format!("/api/owners/{owner_id}/workspaces/w/execute-draft");
    let (_, canonical) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    let mut request = canonical["data"]["collections"][0]["requests"][0].clone();
    request["method"] = json!("POST");
    request["url"] = json!(format!("{url}/echo"));
    request["body_kind"] = json!("json");
    request["body"] = json!(r#"{"draft":true}"#);
    request["query"] = json!([{"id":"q","key":"source","value":"draft","enabled":true}]);
    let payload = json!({"request":request,"collection_id":"c","expected_workspace_revision":canonical["revision"]});
    let (status, response) = call(
        &router,
        "POST",
        &draft_path,
        Some(&editor),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let wire: serde_json::Value = serde_json::from_str(response["body"].as_str().unwrap()).unwrap();
    assert_eq!(wire["query"], "source=draft");
    assert_eq!(wire["body"], r#"{"draft":true}"#);
    let (_, unchanged) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(unchanged["revision"], canonical["revision"]);
    assert_eq!(unchanged["data"], canonical["data"]);
    let mut new_payload = payload.clone();
    new_payload["request"]["id"] = json!("new-unsaved-request");
    new_payload["request"]["auth"] =
        json!({"kind":"inherit","token":"","username":"","password":""});
    new_payload["request"]["body"] = json!(r#"{"from":"{{parent_value}}"}"#);
    let (status, new_response) = call(
        &router,
        "POST",
        &draft_path,
        Some(&editor),
        Some(new_payload),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{new_response}");
    let new_wire: serde_json::Value =
        serde_json::from_str(new_response["body"].as_str().unwrap()).unwrap();
    assert_eq!(new_wire["body"], r#"{"from":"from-parent"}"#);
    assert_eq!(new_wire["parent_auth"], true);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let (status, history) = call(
        &router,
        "GET",
        &format!("/api/owners/{owner_id}/workspaces/w/history"),
        Some(&editor),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{history}");
    assert_eq!(history.as_array().unwrap().len(), 2);
    assert!(!history.to_string().contains("parent-token"));
    let (status, audit) = call(
        &router,
        "GET",
        &format!("/api/owners/{owner_id}/workspaces/w/audit"),
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{audit}");
    assert_eq!(
        audit
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["actor_id"] == editor_id
                && event["action"] == "request.draft.execute")
            .count(),
        2
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &draft_path,
            Some(&viewer),
            Some(payload.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let mut stale = payload.clone();
    stale["expected_workspace_revision"] = json!(0);
    assert_eq!(
        call(&router, "POST", &draft_path, Some(&editor), Some(stale))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let mut collision = payload.clone();
    collision["request"]["id"] = json!("other-request");
    assert_eq!(
        call(&router, "POST", &draft_path, Some(&editor), Some(collision))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let mut missing_collection = payload.clone();
    missing_collection["collection_id"] = json!("missing");
    assert_eq!(
        call(
            &router,
            "POST",
            &draft_path,
            Some(&editor),
            Some(missing_collection),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, foreign_identity) = call(&router, "GET", "/api/auth/me", Some(&foreign), None).await;
    assert_eq!(
        call(
            &router,
            "POST",
            &format!(
                "/api/owners/{}/workspaces/w/execute-draft",
                foreign_identity["id"].as_str().unwrap()
            ),
            Some(&editor),
            Some(payload.clone()),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let mut protocol = payload.clone();
    protocol["request"]["protocol"] = json!({"kind":"sse"});
    assert_eq!(
        call(&router, "POST", &draft_path, Some(&editor), Some(protocol))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    fixture.abort();
}

#[tokio::test]
async fn draft_http_withholds_result_after_cross_instance_role_or_revision_change() {
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("draft-fence.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let first = hosted(settings.clone()).await.unwrap();
    let second = hosted(settings).await.unwrap();
    let owner = register(&first, "fenceowner").await;
    let member = register(&first, "fencemember").await;
    let role_started = Arc::new(tokio::sync::Notify::new());
    let role_release = Arc::new(tokio::sync::Notify::new());
    let revision_started = Arc::new(tokio::sync::Notify::new());
    let revision_release = Arc::new(tokio::sync::Notify::new());
    let (url, fixture) = serve(
        Router::new()
            .route(
                "/slow-role",
                axum::routing::post({
                    let started = role_started.clone();
                    let release = role_release.clone();
                    move || {
                        let started = started.clone();
                        let release = release.clone();
                        async move {
                            started.notify_one();
                            release.notified().await;
                            "private-draft-reply"
                        }
                    }
                }),
            )
            .route(
                "/slow-revision",
                axum::routing::post({
                    let started = revision_started.clone();
                    let release = revision_release.clone();
                    move || {
                        let started = started.clone();
                        let release = release.clone();
                        async move {
                            started.notify_one();
                            release.notified().await;
                            "private-draft-reply"
                        }
                    }
                }),
            ),
    )
    .await;
    let (_, created) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Fence","data":example_data()})),
    )
    .await;
    let (_, invitation) = call(
        &first,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    let (_, accepted) = call(
        &first,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&member),
        Some(json!({"token":invitation["token"]})),
    )
    .await;
    let owner_id = accepted["owner_id"].as_str().unwrap();
    let member_id = accepted["member_id"].as_str().unwrap();
    let draft_path = format!("/api/owners/{owner_id}/workspaces/w/execute-draft");
    let role_path = format!("/api/owners/{owner_id}/workspaces/w/members/{member_id}");
    let mut request = created["data"]["collections"][0]["requests"][0].clone();
    request["method"] = json!("POST");
    request["url"] = json!(format!("{url}/slow-role"));
    request["timeout_ms"] = json!(10000);
    let payload = json!({"request":request,"collection_id":"c","expected_workspace_revision":created["revision"]});
    let first_for_task = first.clone();
    let member_for_task = member.clone();
    let path_for_task = draft_path.clone();
    let payload_for_task = payload.clone();
    let pending_role = tokio::spawn(async move {
        call(
            &first_for_task,
            "POST",
            &path_for_task,
            Some(&member_for_task),
            Some(payload_for_task),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), role_started.notified())
        .await
        .unwrap();
    assert_eq!(
        call(
            &second,
            "PATCH",
            &role_path,
            Some(&owner),
            Some(json!({"role":"viewer","expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, withheld) = tokio::time::timeout(std::time::Duration::from_secs(3), pending_role)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::NOT_FOUND, "{withheld}");
    assert!(!withheld.to_string().contains("private-draft-reply"));
    role_release.notify_one();
    assert_eq!(
        call(
            &second,
            "PATCH",
            &role_path,
            Some(&owner),
            Some(json!({"role":"editor","expected_revision":2}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let first_for_task = first.clone();
    let member_for_task = member.clone();
    let path_for_task = draft_path.clone();
    let mut revision_payload = payload;
    revision_payload["request"]["url"] = json!(format!("{url}/slow-revision"));
    let pending_revision = tokio::spawn(async move {
        call(
            &first_for_task,
            "POST",
            &path_for_task,
            Some(&member_for_task),
            Some(revision_payload),
        )
        .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        revision_started.notified(),
    )
    .await
    .unwrap();
    assert_eq!(
        call(&second, "PUT", &format!("/api/owners/{owner_id}/workspaces/w"), Some(&owner), Some(json!({"name":"Changed","expected_revision":created["revision"],"data":created["data"]}))).await.0,
        StatusCode::OK
    );
    let (status, withheld) =
        tokio::time::timeout(std::time::Duration::from_secs(3), pending_revision)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(status, StatusCode::CONFLICT, "{withheld}");
    assert!(!withheld.to_string().contains("private-draft-reply"));
    revision_release.notify_one();
    fixture.abort();
}

#[tokio::test]
async fn rejoining_with_same_role_cannot_restore_an_in_flight_request() {
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("rejoin-run.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let first = hosted(settings.clone()).await.unwrap();
    let second = hosted(settings).await.unwrap();
    let owner = register(&first, "rejoinowner").await;
    let member = register(&first, "rejoinmember").await;
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (url, server) = serve(Router::new().route(
        "/slow",
        axum::routing::get({
            let started = started.clone();
            let release = release.clone();
            move || {
                let started = started.clone();
                let release = release.clone();
                async move {
                    started.notify_one();
                    release.notified().await;
                    "private-result"
                }
            }
        }),
    ))
    .await;
    let mut source = example_data();
    source["collections"][0]["requests"][0]["url"] = json!(format!("{url}/slow"));
    source["collections"][0]["requests"][0]["timeout_ms"] = json!(10000);
    let (status, _) = call(
        &first,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"Shared","data":source})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, invitation) = call(
        &first,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    let (_, accepted) = call(
        &first,
        "POST",
        "/api/workspace-invitations/accept",
        Some(&member),
        Some(json!({"token":invitation["token"]})),
    )
    .await;
    let owner_id = accepted["owner_id"].as_str().unwrap().to_owned();
    let member_id = accepted["member_id"].as_str().unwrap().to_owned();
    let run_path = format!("/api/owners/{owner_id}/workspaces/w/requests/r/execute");
    let first_for_run = first.clone();
    let member_for_run = member.clone();
    let running = tokio::spawn(async move {
        call(
            &first_for_run,
            "POST",
            &run_path,
            Some(&member_for_run),
            Some(json!({})),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .unwrap();
    let remove_path = format!("/api/owners/{owner_id}/workspaces/w/members/{member_id}");
    assert_eq!(
        call(&second, "DELETE", &remove_path, Some(&owner), None)
            .await
            .0,
        StatusCode::OK
    );
    let (_, second_invitation) = call(
        &second,
        "POST",
        "/api/workspaces/w/invitations",
        Some(&owner),
        Some(json!({"role":"editor","expires_in_days":1})),
    )
    .await;
    assert_eq!(
        call(
            &second,
            "POST",
            "/api/workspace-invitations/accept",
            Some(&member),
            Some(json!({"token":second_invitation["token"]}))
        )
        .await
        .0,
        StatusCode::OK
    );
    release.notify_one();
    let (status, result) = tokio::time::timeout(std::time::Duration::from_secs(5), running)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::NOT_FOUND, "{result}");
    assert!(!result.to_string().contains("private-result"));
    let (_, shared) = call(
        &second,
        "GET",
        "/api/workspaces/shared",
        Some(&member),
        None,
    )
    .await;
    assert_eq!(shared["items"][0]["role"], "editor");
    server.abort();
}

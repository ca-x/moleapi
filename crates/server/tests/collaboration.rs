mod common;
use common::*;

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
    let mut source = data();
    source["global_variables"] =
        json!([{"id":"s","key":"secret","value":"private-sentinel","enabled":true,"secret":true}]);
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
    let (status, updated) = call(
        &router,
        "PUT",
        &shared_path,
        Some(&member),
        Some(json!({"name":"Edited","expected_revision":editor["revision"],"data":editor["data"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
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
    release.notify_one();
    server.abort();
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

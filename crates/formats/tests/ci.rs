use moleapi_formats::ci::{self, Config, Provider, Source};
use serde_json::{Value, json};
fn config() -> Config {
    serde_json::from_value(json!({"provider":"github","source":{"kind":"remote","server":"https://api.example.com/prefix","workspace":"w"},"collection":"Suite $(touch escaped) 'quoted'","environment":"Dev","branches":["main","feature/quote'\"$name"],"secret_name":"MOLEAPI_CI"})).unwrap()
}
#[test]
fn provider_triggers_publish_reports_and_reference_secrets_without_inline_values() {
    let mut config = config();
    let github = ci::generate(&config).unwrap();
    let doc: Value = serde_yaml_ng::from_str(&github.content).unwrap();
    assert!(doc["on"]["push"].is_object() && doc["on"]["pull_request"].is_object());
    assert_eq!(doc["permissions"]["contents"], "read");
    let job = &doc["jobs"]["api-tests"];
    let script = job["steps"][1]["run"].as_str().unwrap();
    let line = script.lines().last().unwrap();
    assert_eq!(shlex::split(line).unwrap(), github.command);
    assert_eq!(
        job["steps"][1]["env"]["MOLEAPI_TOKEN"],
        "${{ secrets.MOLEAPI_CI }}"
    );
    assert_eq!(job["steps"][2]["if"], "always()");
    assert!(job["if"].as_str().unwrap().contains("head.repo.full_name"));
    config.provider = Provider::Gitlab;
    let gitlab = ci::generate(&config).unwrap();
    let doc: Value = serde_yaml_ng::from_str(&gitlab.content).unwrap();
    assert_eq!(doc["moleapi-tests"]["script"][0], script);
    assert_eq!(
        doc["moleapi-tests"]["variables"]["MOLEAPI_TOKEN"],
        "$MOLEAPI_CI"
    );
    assert_eq!(doc["moleapi-tests"]["artifacts"]["when"], "always");
    assert_eq!(
        doc["moleapi-tests"]["artifacts"]["reports"]["junit"],
        ".moleapi-ci/report.xml"
    );
    assert!(
        doc["moleapi-tests"]["rules"][0]["if"]
            .as_str()
            .unwrap()
            .contains("merge_request_event")
    );
    config.secret_name = "MOLEAPI_TOKEN".into();
    let doc: Value = serde_yaml_ng::from_str(&ci::generate(&config).unwrap().content).unwrap();
    assert!(doc["moleapi-tests"].get("variables").is_none());
    config.provider = Provider::Jenkins;
    let jenkins = ci::generate(&config).unwrap();
    assert!(
        jenkins.content.contains("pollSCM(")
            && jenkins.content.contains("withCredentials(")
            && jenkins.content.contains("archiveArtifacts")
    );
    assert!(!jenkins.content.contains("Suite $(touch"));
    config.source = Source::File {
        path: "collection.json".into(),
        format: "postman".into(),
    };
    let local = ci::generate(&config).unwrap();
    assert!(!local.content.contains("withCredentials"));
}
#[test]
fn invalid_configuration_is_rejected_before_emitting_executable_content() {
    for mutate in [
        |c: &mut Config| c.collection = Some("${{ secrets.OTHER }}".into()),
        |c: &mut Config| c.branches = vec!["line\nbreak".into()],
        |c: &mut Config| c.secret_name = "SECRET');execute()".into(),
        |c: &mut Config| c.scenario = Some("s".into()),
        |c: &mut Config| c.iterations = Some(101),
    ] {
        let mut c = config();
        mutate(&mut c);
        assert!(ci::generate(&c).is_err());
    }
    let mut c = config();
    c.source = Source::Remote {
        server: "https://user:password@example.com".into(),
        workspace: "w".into(),
    };
    assert!(ci::generate(&c).is_err());
    let mut value = serde_json::to_value(config()).unwrap();
    value["token"] = json!("never-inline");
    assert!(serde_json::from_value::<Config>(value).is_err());
}

#[test]
fn private_run_variables_bind_by_reference_for_each_provider_and_reject_collisions() {
    let mut config = config();
    config.variables_secret = Some("CI_VARIABLES_JSON".into());
    let github = ci::generate(&config).unwrap();
    let doc: Value = serde_yaml_ng::from_str(&github.content).unwrap();
    assert_eq!(
        doc["jobs"]["api-tests"]["steps"][1]["env"]["MOLEAPI_RUN_VARIABLES"],
        "${{ secrets.CI_VARIABLES_JSON }}"
    );
    assert!(
        github
            .command
            .iter()
            .any(|arg| arg == "--variables-env=MOLEAPI_RUN_VARIABLES")
    );
    config.provider = Provider::Gitlab;
    let doc: Value = serde_yaml_ng::from_str(&ci::generate(&config).unwrap().content).unwrap();
    assert_eq!(
        doc["moleapi-tests"]["variables"]["MOLEAPI_RUN_VARIABLES"],
        "$CI_VARIABLES_JSON"
    );
    config.provider = Provider::Jenkins;
    let preset = ci::generate(&config).unwrap();
    assert!(
        preset
            .content
            .contains("credentialsId: 'CI_VARIABLES_JSON', variable: 'MOLEAPI_RUN_VARIABLES'")
    );
    config.variables_secret = Some(config.secret_name.clone());
    assert!(ci::generate(&config).is_err());
    config.source = Source::File {
        path: "collection.json".into(),
        format: "postman".into(),
    };
    config.variables_secret = Some("MOLEAPI_RUN_VARIABLES".into());
    config.provider = Provider::Gitlab;
    let doc: Value = serde_yaml_ng::from_str(&ci::generate(&config).unwrap().content).unwrap();
    assert!(doc["moleapi-tests"].get("variables").is_none());
}

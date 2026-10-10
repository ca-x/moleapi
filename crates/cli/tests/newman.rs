#[allow(dead_code)]
mod common;
use common::*;
use serde_json::{Value, json};
#[tokio::test]
#[ignore = "requires explicitly installed official Node/Newman fixture runtime"]
async fn upstream_newman_runs_v21_scripts_data_reports_custom_reporter_and_sdk() {
    let node = std::env::var("MOLEAPI_TEST_NODE").expect("MOLEAPI_TEST_NODE");
    let entrypoint = std::env::var("MOLEAPI_TEST_NEWMAN").expect("MOLEAPI_TEST_NEWMAN");
    let (url, fixture) = serve(axum::Router::new().route(
        "/",
        axum::routing::get(
            |axum::extract::Query(query): axum::extract::Query<
                std::collections::BTreeMap<String, String>,
            >| async move { axum::Json(json!(query)) },
        ),
    ))
    .await;
    let temporary = tempfile::tempdir().unwrap();
    let collection = temporary.path().join("collection.json");
    let environment = temporary.path().join("environment.json");
    let data = temporary.path().join("rows.csv");
    let json_report = temporary.path().join("report.json");
    let junit = temporary.path().join("report.xml");
    let custom = temporary.path().join("custom.json");
    let mut source = json!({"info":{"name":"Upstream compatibility","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":[{"name":"Echo","request":{"method":"GET","url":"{{base}}/?row={{row}}"},"event":[{"listen":"prerequest","script":{"type":"text/javascript","exec":["pm.variables.set('local', pm.iterationData.get('row')); pm.environment.set('fromScript', 'ready');"]}},{"listen":"test","script":{"type":"text/javascript","exec":["pm.test('data',()=>pm.expect(pm.response.json().row).to.equal(pm.variables.get('local'))); pm.test('environment',()=>pm.expect(pm.environment.get('fromScript')).to.equal('ready')); "]}}]}]});
    write(&collection, &source);
    write(
        &environment,
        &json!({"name":"Development","values":[{"key":"base","value":url,"enabled":true}]}),
    );
    std::fs::write(&data, "row\none\ntwo\n").unwrap();
    let reporter = temporary
        .path()
        .join("node_modules/newman-reporter-molefixture");
    std::fs::create_dir_all(&reporter).unwrap();
    std::fs::write(reporter.join("index.js"),"module.exports=function(emitter,options){emitter.on('beforeDone',function(err,o){if(err)throw err;emitter.exports.push({name:'fixture',default:'custom.json',path:options.export,content:JSON.stringify(o.summary.run.stats.assertions)});});};").unwrap();
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_moleapi-cli"));
    command
        .args([
            "newman",
            "--node",
            &node,
            "--entrypoint",
            &entrypoint,
            "--",
            "run",
            collection.to_str().unwrap(),
            "-e",
            environment.to_str().unwrap(),
            "-d",
            data.to_str().unwrap(),
            "--reporters",
            "json,junit,molefixture",
            "--reporter-json-export",
            json_report.to_str().unwrap(),
            "--reporter-junit-export",
            junit.to_str().unwrap(),
            "--reporter-molefixture-export",
            custom.to_str().unwrap(),
        ])
        .env("NODE_PATH", temporary.path().join("node_modules"))
        .kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(30), command.output())
        .await
        .unwrap()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&std::fs::read(&json_report).unwrap()).unwrap();
    assert_eq!(report["run"]["stats"]["requests"]["total"], 2);
    assert_eq!(report["run"]["stats"]["assertions"]["failed"], 0);
    let stats: Value = serde_json::from_slice(&std::fs::read(&custom).unwrap()).unwrap();
    assert_eq!(stats["total"], 4);
    assert_eq!(stats["failed"], 0);
    let xml = std::fs::read_to_string(&junit).unwrap();
    roxmltree::Document::parse(&xml).unwrap();
    source["item"][0]["event"][1]["script"]["exec"] =
        json!(["pm.test('failure',()=>pm.expect(1).to.equal(2));"]);
    write(&collection, &source);
    let failed = cli(&[
        "newman",
        "--node",
        &node,
        "--entrypoint",
        &entrypoint,
        "--",
        "run",
        collection.to_str().unwrap(),
        "-e",
        environment.to_str().unwrap(),
        "-d",
        data.to_str().unwrap(),
        "--reporters",
        "cli",
    ])
    .await;
    assert_eq!(failed.status.code(), Some(1));
    let adapter =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/newman/index.cjs");
    let script = "const api=require(process.argv[1]); if(typeof api.newman.run!=='function'||typeof api.collection.Collection!=='function'||typeof api.Runtime.Runner!=='function'||typeof api.transformer.convert!=='function')process.exit(1); const collection=new api.collection.Collection({info:{name:'SDK'},item:[]}); if(!collection.toJSON())process.exit(2); api.newman.run({collection:process.argv[2],environment:process.argv[3],iterationData:process.argv[4],reporters:[]},(err,summary)=>{if(err||summary.run.failures.length!==2||summary.run.stats.requests.total!==2)process.exit(3);});";
    let sdk = tokio::process::Command::new(&node)
        .arg("-e")
        .arg(script)
        .arg(adapter)
        .arg(&collection)
        .arg(&environment)
        .arg(&data)
        .output()
        .await
        .unwrap();
    assert!(
        sdk.status.success(),
        "{}",
        String::from_utf8_lossy(&sdk.stderr)
    );
    fixture.abort();
}

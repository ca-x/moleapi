#![recursion_limit = "256"]
use moleapi_core::*;
use serde_json::json;
fn report() -> SavedRunReport {
    serde_json::from_value(json!({"schema_version":1,"id":"12345678-1234-4123-8123-123456789abc","workspace_id":"w","workspace_name":"=SUM(1) 猫 <script>unsafe</script>","workspace_revision":1,"collection_id":"c","collection_name":"Collection & <suite>","scenario_id":null,"scenario_name":null,"environment_id":null,"environment_name":null,"dataset_id":null,"started_at":"2026-10-10T00:00:00Z","finished_at":"2026-10-10T00:00:01Z","summary":{"passed":0,"failed":1,"skipped":0,"elapsed_ms":1000,"iteration_count":1,"completed_iterations":1,"executed_steps":1,"tests_passed":0,"tests_failed":1,"diagnostics_omitted":0,"stopped_reason":null,"cancelled":false},"iterations":[],"results":[{"position":0,"request_id":"r","request_name":" \t=HYPERLINK(\"x\")","collection_id":"c","method":"GET","iteration":0,"step_id":null,"step_name":"Unicode 猫 <&> \"quotes\"","step_group":"+SUM(1)","parallel_id":null,"parallel_name":null,"repeat_index":0,"outcome":"failed","status":200,"elapsed_ms":1,"size_bytes":1,"tests":[{"id":"t","name":"Check 猫 <&>","passed":false,"actual":"<img src=x onerror=oops> & 雪","expected":"2"}],"tests_passed":0,"tests_failed":1,"diagnostics_omitted":0,"error":null}]})).unwrap()
}
#[test]
fn mature_exporters_roundtrip_junit_csv_and_escape_bilingual_html() {
    let report = report();
    let file = moleapi_formats::export_run_report(&report, "junit", "en").unwrap();
    let restored = quick_junit::Report::deserialize_from_str(&file.content).unwrap();
    assert_eq!(restored.tests, 1);
    assert_eq!(restored.failures, 1);
    assert!(
        restored.test_suites[0].test_cases[0]
            .name
            .as_str()
            .contains("Unicode 猫 <&>")
    );
    let file = moleapi_formats::export_run_report(&report, "csv", "en").unwrap();
    let mut reader = csv::Reader::from_reader(file.content.as_bytes());
    let row = reader.records().next().unwrap().unwrap();
    assert_eq!(&row[3], "' \t=HYPERLINK(\"x\")");
    assert_eq!(&row[8], "'+SUM(1)");
    for (language, title) in [("en", "API test report"), ("zh-CN", "API 测试报告")] {
        let file = moleapi_formats::export_run_report(&report, "html", language).unwrap();
        assert!(file.content.contains(title));
        assert!(!file.content.contains("<script>unsafe</script>"));
        assert!(!file.content.contains("<img src=x onerror=oops>"));
        assert!(file.content.contains("&lt;img src=x onerror=oops&gt;"));
    }
}
#[test]
fn empty_cancelled_and_parallel_conflict_reports_cannot_pass_junit_as_empty_success() {
    for reason in [None, Some("cancelled"), Some("parallel_variable_conflict")] {
        let mut report = report();
        report.results.clear();
        report.summary.stopped_reason = reason.map(str::to_string);
        let file = moleapi_formats::export_run_report(&report, "junit", "en").unwrap();
        let restored = quick_junit::Report::deserialize_from_str(&file.content).unwrap();
        assert_eq!(restored.errors, 1);
        assert_eq!(restored.tests, 1);
    }
    assert!(moleapi_formats::export_run_report(&report(), "pdf", "en").is_err());
}

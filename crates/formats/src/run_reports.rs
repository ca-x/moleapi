pub struct RunReportPrivacy(crate::redact::ExportPrivacy);
impl RunReportPrivacy {
    pub fn new(workspace: &moleapi_core::Workspace) -> Self {
        Self(crate::redact::ExportPrivacy::from_workspace(
            workspace, true,
        ))
    }
    pub fn screen(&self, text: &str, limit: usize) -> String {
        self.0.screen_bounded(text, limit)
    }
}
use crate::ExportResult;
use anyhow::{Result, bail, ensure};
use moleapi_core::{RunOutcome, SavedRunReport};
use quick_junit::{NonSuccessKind, Report, TestCase, TestCaseStatus, TestSuite};
use std::{collections::BTreeMap, time::Duration};
pub fn export_run_report(
    report: &SavedRunReport,
    format: &str,
    language: &str,
) -> Result<ExportResult> {
    ensure!(
        matches!(language, "en" | "zh-CN"),
        "Unsupported report language"
    );
    ensure!(
        report.schema_version == 1
            && report.results.len() <= 1000
            && serde_json::to_vec(report)?.len() <= 4 * 1024 * 1024,
        "Report exceeds export limits"
    );
    let id = uuid::Uuid::parse_str(&report.id)?.simple();
    let (extension, mime, content) = match format {
        "json" => (
            "json",
            "application/json",
            serde_json::to_string_pretty(report)?,
        ),
        "csv" => ("csv", "text/csv; charset=utf-8", csv(report)?),
        "junit" => ("xml", "application/xml", junit(report)?),
        "html" => ("html", "text/html; charset=utf-8", html(report, language)),
        _ => bail!("Report format must be json, csv, junit or html"),
    };
    ensure!(
        content.len() <= 16 * 1024 * 1024,
        "Report export exceeds 16 MiB"
    );
    Ok(ExportResult {
        warnings: vec![],
        filename: format!("moleapi-report-{id}.{extension}"),
        content,
        mime: mime.into(),
    })
}
fn outcome(value: &RunOutcome) -> &'static str {
    match value {
        RunOutcome::Passed => "passed",
        RunOutcome::Failed => "failed",
        RunOutcome::Skipped => "skipped",
    }
}
fn csv_text(value: &str) -> String {
    let leading =
        value.trim_start_matches(|c: char| c.is_whitespace() || c.is_control() || c == '\u{feff}');
    if leading.starts_with(['=', '+', '-', '@']) {
        format!("'{value}")
    } else {
        value.into()
    }
}
fn csv(report: &SavedRunReport) -> Result<String> {
    let mut writer = ::csv::Writer::from_writer(Vec::new());
    writer.write_record([
        "position",
        "iteration",
        "request_id",
        "request_name",
        "collection_id",
        "method",
        "step_id",
        "step_name",
        "group",
        "parallel_id",
        "parallel_name",
        "repeat_index",
        "outcome",
        "status",
        "elapsed_ms",
        "size_bytes",
        "tests_passed",
        "tests_failed",
        "diagnostics_omitted",
        "error",
    ])?;
    for row in &report.results {
        writer.write_record([
            row.position.to_string(),
            row.iteration.to_string(),
            csv_text(&row.request_id),
            csv_text(&row.request_name),
            csv_text(&row.collection_id),
            csv_text(&row.method),
            csv_text(row.step_id.as_deref().unwrap_or("")),
            csv_text(row.step_name.as_deref().unwrap_or("")),
            csv_text(row.step_group.as_deref().unwrap_or("")),
            csv_text(row.parallel_id.as_deref().unwrap_or("")),
            csv_text(row.parallel_name.as_deref().unwrap_or("")),
            row.repeat_index.to_string(),
            outcome(&row.outcome).into(),
            row.status.map_or(String::new(), |value| value.to_string()),
            row.elapsed_ms.to_string(),
            row.size_bytes.to_string(),
            row.tests_passed.to_string(),
            row.tests_failed.to_string(),
            row.diagnostics_omitted.to_string(),
            csv_text(row.error.as_deref().unwrap_or("")),
        ])?;
    }
    Ok(String::from_utf8(writer.into_inner()?)?)
}
fn junit(report: &SavedRunReport) -> Result<String> {
    let mut output = Report::new(
        report
            .scenario_name
            .as_ref()
            .unwrap_or(&report.collection_name)
            .as_str(),
    );
    output.set_time(Duration::from_millis(report.summary.elapsed_ms));
    let mut suites = BTreeMap::<usize, TestSuite>::new();
    for row in &report.results {
        let mut status = match row.outcome {
            RunOutcome::Passed => TestCaseStatus::success(),
            RunOutcome::Skipped => TestCaseStatus::skipped(),
            RunOutcome::Failed => TestCaseStatus::non_success(if row.error.is_some() {
                NonSuccessKind::Error
            } else {
                NonSuccessKind::Failure
            }),
        };
        if row.outcome == RunOutcome::Failed {
            let message = row
                .error
                .clone()
                .unwrap_or_else(|| format!("{} failed assertions", row.tests_failed));
            status.set_message(message);
            status.set_description(
                row.tests
                    .iter()
                    .filter(|test| !test.passed)
                    .map(|test| {
                        format!(
                            "{}: actual={}, expected={}",
                            test.name, test.actual, test.expected
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        let name = format!(
            "{} [step {}, repeat {}]",
            row.step_name
                .as_ref()
                .filter(|name| !name.is_empty())
                .unwrap_or(&row.request_name),
            row.position + 1,
            row.repeat_index + 1
        );
        let mut case = TestCase::new(name, status);
        case.set_time(Duration::from_millis(row.elapsed_ms));
        case.set_classname(row.collection_id.as_str());
        suites
            .entry(row.iteration)
            .or_insert_with(|| {
                TestSuite::new(format!(
                    "{} · iteration {}",
                    report.collection_name,
                    row.iteration + 1
                ))
            })
            .add_test_case(case);
    }
    for suite in suites.into_values() {
        output.add_test_suite(suite);
    }
    if let Some(reason) = report
        .summary
        .stopped_reason
        .as_deref()
        .or_else(|| report.results.is_empty().then_some("empty_run"))
    {
        let mut status = TestCaseStatus::non_success(NonSuccessKind::Error);
        status.set_message(reason);
        let mut suite = TestSuite::new("run-control");
        suite.add_test_case(TestCase::new("Run did not complete normally", status));
        output.add_test_suite(suite);
    }
    Ok(output.to_string()?)
}
fn label<'a>(language: &str, en: &'a str, zh: &'a str) -> &'a str {
    if language == "zh-CN" { zh } else { en }
}
fn html(report: &SavedRunReport, language: &str) -> String {
    use maud::html;
    let title = label(language, "API test report", "API 测试报告");
    html!{
  (maud::DOCTYPE) html lang=(language){head{meta charset="utf-8";meta name="viewport" content="width=device-width, initial-scale=1";title{(title)}style{"body{font-family:system-ui,sans-serif;color:#202322;background:#fafaf8;margin:32px auto;padding:0 20px;max-width:1180px;line-height:1.5}h1{font-size:26px;margin-bottom:6px}h2{font-size:18px;margin-top:28px}.facts{display:flex;flex-wrap:wrap;gap:16px;color:#555}table{width:100%;border-collapse:collapse;font-size:14px}td,th{text-align:left;vertical-align:top;padding:10px 8px;border-bottom:1px solid #ddd;overflow-wrap:anywhere}th{font-weight:600}.failed{color:#a32020}.passed{color:#20643b}.skipped{color:#666}pre{white-space:pre-wrap;overflow-wrap:anywhere}summary{cursor:pointer}small{color:#666}@media print{body{background:white;margin:0}details{break-inside:avoid}}"}}
  body{h1{(title)}p{(&report.workspace_name)" / "(&report.collection_name) @if let Some(name)=&report.scenario_name{" / "(name)}}
   div class="facts"{span{(label(language,"Source revision","来源版本"))": "(report.workspace_revision)}span{(&report.started_at)}span{(report.summary.elapsed_ms)" ms"}}
   p{(label(language,"Passed","通过"))": "(report.summary.passed)" · "(label(language,"Failed","失败"))": "(report.summary.failed)" · "(label(language,"Skipped","跳过"))": "(report.summary.skipped)}
   @if let Some(reason)=&report.summary.stopped_reason{p class="failed"{(label(language,"Stopped","流程停止"))": "(reason)}}
   p{small{(label(language,"This saved report excludes live bodies, headers, logs and variable values. Diagnostic omissions are counted explicitly.","保存报告不包含即时响应正文、响应头、日志或变量值。省略的断言详情会单独计数。"))}}
   table{thead{tr{th{(label(language,"Step","步骤"))}th{(label(language,"Request","请求"))}th{(label(language,"Outcome / status","结果 / 状态"))}th{(label(language,"Time","耗时"))}th{(label(language,"Assertions","断言"))}}}tbody{
    @for row in &report.results{tr{td{(row.position+1)" · "(label(language,"iteration","轮次"))" "(row.iteration+1) @if row.repeat_index>0{" · "(label(language,"repeat","重复"))" "(row.repeat_index+1)}}
     td{(&row.method)" "(row.step_name.as_ref().filter(|name|!name.is_empty()).unwrap_or(&row.request_name)) @if let Some(name)=&row.parallel_name{br;small{(name)}}}
     td class=(outcome(&row.outcome)){(label(language,outcome(&row.outcome),match row.outcome{RunOutcome::Passed=>"通过",RunOutcome::Failed=>"失败",RunOutcome::Skipped=>"跳过"})) @if let Some(status)=row.status{" / "(status)} @if let Some(error)=&row.error{p{(error)}}}
     td{(row.elapsed_ms)" ms"}td{(row.tests_passed)" / "(row.tests_passed+row.tests_failed)
      @if row.diagnostics_omitted>0{p{small{(label(language,"Omitted details","省略详情"))": "(row.diagnostics_omitted)}}}
      @for test in &row.tests{details{summary class=(if test.passed{"passed"}else{"failed"}){(&test.name)}pre{(label(language,"Actual","实际值"))": "(&test.actual)"\n"(label(language,"Expected","期望值"))": "(&test.expected)}}}
     }
    }}
   }}
  }}
 }.into_string()
}

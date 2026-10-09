//! Bounded QuickJS runtime with the deliberately limited `moleapi-pm/1` API.
//! No filesystem, native modules, timers, processes or independent networking.
use anyhow::{Context as _, Result, ensure};
use moleapi_core::{RequestSpec, Response, ScriptLog, TestResult, VariableScopes, VariableUpdate};
use rquickjs::{Context, Ctx, Exception, Function, Runtime, Value};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
    time::{Duration, Instant},
};
mod failure;
mod worker;
pub use failure::ScriptFailure;
pub use worker::*;

pub const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
pub const DEADLINE: Duration = Duration::from_millis(250);
pub const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
#[derive(Serialize)]
struct Input<'a> {
    request: &'a RequestSpec,
    response: Option<&'a Response>,
    scopes: &'a VariableScopes,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ScriptOutput {
    #[serde(default)]
    pub control: moleapi_core::ExecutionControl,
    pub request: RequestSpec,
    pub logs: Vec<ScriptLog>,
    pub tests: Vec<TestResult>,
    pub updates: Vec<VariableUpdate>,
    #[serde(default)]
    pub private_values: BTreeSet<String>,
}

fn capture_private(
    ctx: &Ctx<'_>,
    values: &RefCell<BTreeSet<String>>,
    complete: &Cell<bool>,
    value: String,
) -> rquickjs::Result<()> {
    let mut values = values.borrow_mut();
    if !values.contains(&value)
        && (values.len() >= 5000
            || values
                .iter()
                .map(String::len)
                .sum::<usize>()
                .saturating_add(value.len())
                > 4 * moleapi_core::MAX_VARIABLE_BYTES)
    {
        complete.set(false);
        return Err(Exception::throw_range(
            ctx,
            "Private variable history exceeds execution limit",
        ));
    }
    values.insert(value);
    Ok(())
}
fn capture_url(
    ctx: &Ctx<'_>,
    values: &RefCell<BTreeSet<String>>,
    complete: &Cell<bool>,
    raw: &str,
) -> rquickjs::Result<()> {
    if raw.len() > moleapi_core::MAX_BODY {
        complete.set(false);
        return Err(Exception::throw_range(
            ctx,
            "Request privacy URL exceeds capture limit",
        ));
    }
    let Ok(url) = url::Url::parse(raw) else {
        return Ok(());
    };
    let decode = |value: &str| {
        percent_encoding::percent_decode_str(value)
            .decode_utf8_lossy()
            .into_owned()
    };
    if !url.username().is_empty() {
        capture_private(ctx, values, complete, url.username().into())?;
        capture_private(ctx, values, complete, decode(url.username()))?;
    }
    if let Some(password) = url.password() {
        capture_private(ctx, values, complete, password.into())?;
        capture_private(ctx, values, complete, decode(password))?;
        capture_private(
            ctx,
            values,
            complete,
            format!("{}:{}", decode(url.username()), decode(password)),
        )?;
    }
    for (index, (key, value)) in url.query_pairs().enumerate() {
        if index >= 1000 {
            complete.set(false);
            return Err(Exception::throw_range(
                ctx,
                "Request privacy URL exceeds 1000 query fields",
            ));
        }
        if moleapi_core::sensitive_query_key(&key) {
            capture_private(ctx, values, complete, value.into_owned())?;
        }
    }
    Ok(())
}

pub fn run(
    scripts: &[String],
    request: &RequestSpec,
    response: Option<&Response>,
    scopes: &VariableScopes,
) -> Result<ScriptOutput> {
    for script in scripts {
        moleapi_core::validate_script(script)?;
    }
    scopes.validate()?;
    let input = serde_json::to_string(&Input {
        request,
        response,
        scopes,
    })?;
    ensure!(
        input.len() <= 16 * 1024 * 1024,
        "Script input exceeds 16 MiB"
    );
    let private_values = Rc::new(RefCell::new(scopes.private_values.clone()));
    let privacy_complete = Rc::new(Cell::new(true));
    let failure_privacy_complete = Rc::new(Cell::new(true));
    let active_capture = Rc::new(Cell::new(0usize));
    let result = (|| -> Result<ScriptOutput> {
        let runtime = Runtime::new().context("Create JavaScript runtime")?;
        runtime.set_memory_limit(MEMORY_LIMIT);
        runtime.set_max_stack_size(512 * 1024);
        let deadline = Instant::now() + DEADLINE;
        runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
        let promise_seen = Rc::new(Cell::new(false));
        let promise_flag = promise_seen.clone();
        runtime.set_promise_hook(Some(Box::new(move |_, _, _, _| promise_flag.set(true))));
        let context = Context::full(&runtime).context("Create JavaScript context")?;
        let output = context.with(|ctx| {
            let skipped = Rc::new(Cell::new(false));
            let skip_flag = skipped.clone();
            ctx.globals().set(
                "__moleapiSkipRequest",
                Function::new(ctx.clone(), move || {
                    skip_flag.set(true);
                })?,
            )?;
            let evaluate = |source: &str| -> Result<Value<'_>> {
                ctx.eval(source).map_err(|error| {
                    let detail = if error.is_exception() {
                        let caught = ctx.catch();
                        // Never format arbitrary script objects through user-defined callbacks.
                        if let Some(object) = caught.as_object() {
                            object
                                .get::<_, String>("message")
                                .unwrap_or_else(|_| "JavaScript exception".into())
                        } else {
                            "JavaScript exception".into()
                        }
                    } else {
                        error.to_string()
                    };
                    let detail: String = detail.chars().take(4096).collect();
                    anyhow::anyhow!("JavaScript error: {detail}")
                })
            };
            let values = private_values.clone();
            let complete = privacy_complete.clone();
            let capture = Function::new(
                ctx.clone(),
                move |ctx: Ctx<'_>, value: String| -> rquickjs::Result<()> {
                    capture_private(&ctx, &values, &complete, value)
                },
            )?;
            ctx.globals().set("__moleapiTaint", capture)?;
            let uncertain = failure_privacy_complete.clone();
            ctx.globals().set(
                "__moleapiPrivacyUncertain",
                Function::new(ctx.clone(), move || {
                    uncertain.set(false);
                })?,
            )?;
            let active = active_capture.clone();
            ctx.globals().set(
                "__moleapiCaptureBegin",
                Function::new(ctx.clone(), move || {
                    active.set(active.get().saturating_add(1));
                })?,
            )?;
            let active = active_capture.clone();
            let failure_complete = failure_privacy_complete.clone();
            let complete = privacy_complete.clone();
            ctx.globals().set(
                "__moleapiCaptureEnd",
                Function::new(ctx.clone(), move |success: bool| {
                    if !success {
                        failure_complete.set(false);
                        complete.set(false);
                    }
                    active.set(active.get().saturating_sub(1));
                })?,
            )?;
            let complete = privacy_complete.clone();
            ctx.globals().set(
                "__moleapiPrivacyOverflow",
                Function::new(ctx.clone(), move |ctx: Ctx<'_>| -> rquickjs::Result<()> {
                    complete.set(false);
                    Err(Exception::throw_range(
                        &ctx,
                        "Request privacy capture work limit exceeded",
                    ))
                })?,
            )?;
            let values = private_values.clone();
            let complete = privacy_complete.clone();
            let query_values = private_values.clone();
            let query_complete = privacy_complete.clone();
            ctx.globals().set(
                "__moleapiCaptureQuery",
                Function::new(
                    ctx.clone(),
                    move |ctx: Ctx<'_>,
                          key: String,
                          value: String,
                          secret: bool|
                          -> rquickjs::Result<bool> {
                        if key.len() > moleapi_core::MAX_BODY
                            || value.len() > moleapi_core::MAX_BODY
                        {
                            query_complete.set(false);
                            return Err(Exception::throw_range(
                                &ctx,
                                "Request privacy query field exceeds capture limit",
                            ));
                        }
                        let sensitive = secret || moleapi_core::sensitive_query_key(&key);
                        if sensitive {
                            capture_private(&ctx, &query_values, &query_complete, value)?;
                        }
                        Ok(sensitive)
                    },
                )?,
            )?;
            ctx.globals().set(
                "__moleapiCaptureUrl",
                Function::new(
                    ctx.clone(),
                    move |ctx: Ctx<'_>, raw: String| -> rquickjs::Result<()> {
                        capture_url(&ctx, &values, &complete, &raw)
                    },
                )?,
            )?;
            // Bind serialized input as a JS string and parse JSON: evaluating JSON as
            // an object literal would give __proto__ keys different semantics.
            ctx.globals()
                .set("__moleapiInput", input.as_str())
                .context("Bind script input")?;
            evaluate(&format!(
                "{}(JSON.parse(__moleapiInput))",
                include_str!("pm.js")
            ))?;
            ctx.globals()
                .remove("__moleapiInput")
                .context("Release script input")?;
            ctx.globals().remove("__moleapiTaint")?;
            for name in [
                "__moleapiPrivacyUncertain",
                "__moleapiPrivacyOverflow",
                "__moleapiCaptureUrl",
                "__moleapiCaptureQuery",
                "__moleapiCaptureBegin",
                "__moleapiCaptureEnd",
                "__moleapiSkipRequest",
            ] {
                ctx.globals().remove(name)?;
            }
            for script in scripts.iter().filter(|s| !s.trim().is_empty()) {
                let result = match evaluate(script) {
                    Ok(result) => result,
                    Err(_) if skipped.get() => break,
                    Err(error) => return Err(error),
                };
                ensure!(
                    !result.is_promise() && !promise_seen.get(),
                    "Asynchronous scripts are unsupported in pm compatibility v1"
                );
                if skipped.get() {
                    break;
                }
            }
            let output: String = ctx.eval("__moleapiExport()").map_err(|_| {
                anyhow::anyhow!("JavaScript output exceeded limits or execution deadline")
            })?;
            ensure!(
                output.len() <= MAX_OUTPUT_BYTES,
                "Script output exceeds 8 MiB"
            );
            let mut output: ScriptOutput =
                serde_json::from_str(&output).context("Invalid JavaScript output")?;
            ensure!(
                output.logs.len() <= 200
                    && output.tests.len() <= 200
                    && output.updates.len() <= 1000,
                "Script result count exceeds limits"
            );
            let text_bytes: usize = output
                .logs
                .iter()
                .map(|log| log.message.len())
                .sum::<usize>()
                + output
                    .tests
                    .iter()
                    .map(|test| test.name.len() + test.actual.len() + test.expected.len())
                    .sum::<usize>();
            ensure!(text_bytes <= 64 * 1024, "Script output text exceeds 64 KiB");
            // Validate the complete mutation before handing it to the network executor.
            moleapi_core::validate_request(&output.request, true)?;
            let mut validated = scopes.clone();
            validated.apply(&output.updates)?;
            output.private_values = private_values.borrow().clone();
            Ok(output)
        })?;
        ensure!(
            privacy_complete.get() && active_capture.get() == 0,
            "Private variable history exceeds execution limit"
        );
        ensure!(
            !runtime.is_job_pending(),
            "Asynchronous scripts are unsupported in pm compatibility v1"
        );
        Ok(output)
    })();
    result.map_err(|error| {
        anyhow::Error::new(ScriptFailure {
            message: error.to_string(),
            private_values: private_values.borrow().clone(),
            privacy_complete: privacy_complete.get()
                && failure_privacy_complete.get()
                && active_capture.get() == 0,
        })
    })
}

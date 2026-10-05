use crate::{Command, EventMessage, SendMessage, Session, SessionState};
use anyhow::{Context, Result, ensure};
use moleapi_core::{Environment, NetworkPolicy, Protocol, RequestSpec};
use moleapi_data::{Connection, QueryResult};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::{Semaphore, mpsc};
use tokio_util::sync::CancellationToken;
#[derive(Default)]
pub(crate) struct Control {
    pub environment: Option<Environment>,
    pub worker: Option<PathBuf>,
    active: Option<(String, CancellationToken)>,
    commands: usize,
}
fn slots() -> Arc<Semaphore> {
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SLOTS.get_or_init(|| Arc::new(Semaphore::new(4))).clone()
}
pub(crate) fn send(session: &Arc<Session>, message: SendMessage) -> Result<()> {
    let mut record = session.record.lock().unwrap();
    ensure!(
        record.summary.protocol == "data"
            && record.summary.state == SessionState::Open
            && !session.cancel.is_cancelled(),
        "Data connection is not open"
    );
    let mut control = session.data.lock().unwrap();
    match message {
        SendMessage::DataCancel { query_id } => {
            if let Some((id, stop)) = &control.active {
                ensure!(*id == query_id, "Unknown active Data query");
                stop.cancel();
            }
            Ok(())
        }
        SendMessage::DataQuery {
            query_id,
            sql,
            read_only,
        } => {
            ensure!(
                !query_id.is_empty()
                    && query_id.len() <= 128
                    && !query_id.chars().any(char::is_control),
                "Invalid Data query ID"
            );
            ensure!(
                !sql.trim().is_empty() && sql.len() <= moleapi_core::MAX_SQL_SOURCE,
                "Data SQL query exceeds limits"
            );
            ensure!(control.active.is_none(), "A Data query is already running");
            ensure!(control.commands < 512, "Data command budget exceeded");
            ensure!(
                record.input.saturating_add(sql.len()) <= 20 * 1024 * 1024,
                "Data command input budget exceeded"
            );
            let input_bytes = sql.len();
            let stop = CancellationToken::new();
            session
                .commands
                .try_send(Command::DataQuery {
                    query_id: query_id.clone(),
                    sql,
                    read_only,
                    stop: stop.clone(),
                })
                .context("Data command queue is full")?;
            record.input += input_bytes;
            control.active = Some((query_id, stop));
            control.commands += 1;
            Ok(())
        }
        SendMessage::DataSchemaRefresh => {
            ensure!(control.active.is_none(), "A Data query is already running");
            ensure!(control.commands < 512, "Data command budget exceeded");
            session
                .commands
                .try_send(Command::DataSchema)
                .context("Data command queue is full")?;
            control.active = Some(("schema-refresh".into(), CancellationToken::new()));
            control.commands += 1;
            Ok(())
        }
        _ => anyhow::bail!("Unsupported Data command"),
    }
}
fn active_done(session: &Session) {
    session.data.lock().unwrap().active = None;
}
fn schema_events(
    session: &Session,
    tables: Vec<moleapi_data::SchemaTable>,
    truncated: bool,
) -> Result<()> {
    if tables.is_empty() {
        return session.event(
            "received",
            EventMessage::DataSchema {
                tables: vec![],
                clear: true,
                done: true,
                truncated,
            },
        );
    }
    let mut chunk = Vec::new();
    let mut bytes = 0;
    let mut first = true;
    for table in tables {
        ensure!(!session.cancel.is_cancelled(), "Data connection closed");
        let size = serde_json::to_vec(&table)?.len();
        if bytes + size > 700 * 1024 && !chunk.is_empty() {
            session.event(
                "received",
                EventMessage::DataSchema {
                    tables: std::mem::take(&mut chunk),
                    clear: first,
                    done: false,
                    truncated,
                },
            )?;
            first = false;
            bytes = 0;
        }
        bytes += size;
        chunk.push(table);
    }
    session.event(
        "received",
        EventMessage::DataSchema {
            tables: chunk,
            clear: first,
            done: true,
            truncated,
        },
    )?;
    Ok(())
}
fn result_events(session: &Session, id: &str, result: QueryResult) -> Result<()> {
    session.event(
        "received",
        EventMessage::DataColumns {
            query_id: id.into(),
            columns: result.columns,
        },
    )?;
    let mut chunk = Vec::new();
    let mut bytes = 0;
    for row in result.rows {
        ensure!(!session.cancel.is_cancelled(), "Data connection closed");
        let size = serde_json::to_vec(&row)?.len();
        if bytes + size > 700 * 1024 && !chunk.is_empty() {
            session.event(
                "received",
                EventMessage::DataRows {
                    query_id: id.into(),
                    rows: std::mem::take(&mut chunk),
                },
            )?;
            bytes = 0;
        }
        bytes += size;
        chunk.push(row);
    }
    if !chunk.is_empty() {
        session.event(
            "received",
            EventMessage::DataRows {
                query_id: id.into(),
                rows: chunk,
            },
        )?;
    }
    session.event(
        "received",
        EventMessage::DataFinished {
            query_id: id.into(),
            rows_affected: result.rows_affected,
            elapsed_ms: result.elapsed_ms,
            truncated: result.truncated,
            limit_reason: result.limit_reason,
        },
    )
}
pub(crate) async fn run(
    session: Arc<Session>,
    request: RequestSpec,
    policy: NetworkPolicy,
    mut commands: mpsc::Receiver<Command>,
    mask: Arc<dyn Fn(&str) -> String + Send + Sync>,
) -> Result<String> {
    let Protocol::Data { config } = &request.protocol else {
        anyhow::bail!("Expected Data request")
    };
    let (environment, worker) = {
        let control = session.data.lock().unwrap();
        (
            control
                .environment
                .clone()
                .context("Missing Data environment")?,
            control
                .worker
                .clone()
                .context("Missing trusted Data worker")?,
        )
    };
    let duration = Duration::from_millis(request.timeout_ms);
    let permit = slots()
        .try_acquire_owned()
        .context("Data execution capacity reached")?;
    let mut connection = tokio::select! {biased;_=session.cancel.cancelled()=>return Ok("Data closed during connection".into()),value=tokio::time::timeout(duration,Connection::open(&request,config,policy,&worker))=>value.context("Data connection timed out")??};
    let (tables, truncated) = tokio::select! {biased;_=session.cancel.cancelled()=>return Ok("Data closed during schema discovery".into()),value=tokio::time::timeout(duration,connection.schema())=>value.context("Data schema discovery timed out")??};
    session.event(
        "received",
        EventMessage::DataReady {
            info: connection.info(),
        },
    )?;
    schema_events(&session, tables, truncated)?;
    session.record.lock().unwrap().summary.state = SessionState::Open;
    session.event(
        "system",
        EventMessage::State {
            state: SessionState::Open,
            reason: None,
        },
    )?;
    drop(permit);
    loop {
        let command = tokio::select! {biased;_=session.cancel.cancelled()=>return Ok("Data connection closed".into()),command=commands.recv()=>command.context("Data command channel closed")?};
        match command {
            Command::DataSchema => {
                let _permit = slots()
                    .try_acquire_owned()
                    .context("Data execution capacity reached")?;
                let result = tokio::select! {biased;_=session.cancel.cancelled()=>return Ok("Data closed during schema refresh".into()),result=tokio::time::timeout(duration,connection.schema())=>result.context("Data schema refresh timed out")?};
                active_done(&session);
                match result {
                    Ok((tables, truncated)) => schema_events(&session, tables, truncated)?,
                    Err(error) => session.event(
                        "received",
                        EventMessage::DataError {
                            query_id: String::new(),
                            message: mask(&error.to_string()),
                        },
                    )?,
                }
            }
            Command::DataQuery {
                query_id,
                sql,
                read_only,
                stop,
            } => {
                let permit = match slots().try_acquire_owned() {
                    Ok(value) => value,
                    Err(_) => {
                        active_done(&session);
                        session.event(
                            "received",
                            EventMessage::DataError {
                                query_id,
                                message: "Data execution capacity reached".into(),
                            },
                        )?;
                        continue;
                    }
                };
                let sql = match moleapi_core::resolve_value(&sql, &environment) {
                    Ok(value) => value,
                    Err(error) => {
                        active_done(&session);
                        session.event(
                            "received",
                            EventMessage::DataError {
                                query_id,
                                message: mask(&error.to_string()),
                            },
                        )?;
                        continue;
                    }
                };
                session.event(
                    "received",
                    EventMessage::DataStarted {
                        query_id: query_id.clone(),
                    },
                )?;
                let cancel = connection.cancellation();
                let result = tokio::select! {biased;_=session.cancel.cancelled()=>None,_=stop.cancelled()=>None,result=tokio::time::timeout(duration,connection.query(&sql,read_only,duration))=>Some(result)};
                active_done(&session);
                match result {
                    None => {
                        let _ =
                            tokio::time::timeout(Duration::from_secs(1), cancel.request()).await;
                        if !session.cancel.is_cancelled() {
                            session.event(
                                "received",
                                EventMessage::DataCancelled {
                                    query_id,
                                    write_outcome_unknown: !read_only,
                                },
                            )?;
                        }
                        drop(connection);
                        drop(permit);
                        return Ok("Data query cancellation requested; connection closed".into());
                    }
                    Some(Err(_)) => {
                        let _ =
                            tokio::time::timeout(Duration::from_secs(1), cancel.request()).await;
                        session.event("received",EventMessage::DataError {query_id,message:"Data query timed out; connection closed; write outcome may be unknown".into()})?;
                        drop(connection);
                        drop(permit);
                        return Ok("Data query timed out".into());
                    }
                    Some(Ok(Err(error))) => session.event(
                        "received",
                        EventMessage::DataError {
                            query_id,
                            message: mask(&error.to_string()),
                        },
                    )?,
                    Some(Ok(Ok(result))) => result_events(&session, &query_id, result)?,
                }
                if connection.is_closed() {
                    drop(connection);
                    drop(permit);
                    return Ok("Data transport closed after a capped or failed query; reconnect before running another query. Uncommitted write outcome is not confirmed.".into());
                }
            }
            _ => anyhow::bail!("Unexpected Data command"),
        }
    }
}

//! Real a2a-rs 0.10 server adapter and upstream echo business handler.
#[path = "handler.rs"]
mod handler;
use a2a_rs::adapter::{JsonRpcAdapter, SimpleAgentInfo, jsonrpc_router, rest_router};
use a2a_rs::services::server::AgentInfoProvider;
use axum::{Json, Router, routing::get};
use std::sync::Arc;
pub fn router(base: &str) -> Router {
    let info = SimpleAgentInfo::new("MoleAPI SDK agent".into(), base.into())
        .with_streaming()
        .with_push_notifications()
        .with_preferred_transport("JSONRPC".into())
        .add_interface(base.into(), "HTTP+JSON".into())
        .add_skill(
            "echo".into(),
            "Echo".into(),
            Some("Echo parts".into()),
            vec!["echo".into()],
        );
    let handler = handler::SimpleAgentHandler::new();
    let adapter = Arc::new(
        JsonRpcAdapter::with_handler(handler.clone(), info.clone()).with_streaming_handler(handler),
    );
    jsonrpc_router(adapter.clone())
        .merge(rest_router(adapter))
        .route(
            "/.well-known/agent-card.json",
            get(move || {
                let info = info.clone();
                async move { Json(info.get_agent_card().await.unwrap()) }
            }),
        )
}

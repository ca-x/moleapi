//! Legacy fixture uses upstream a2a-types0.1 models; real a2a-client0.1 handles client wire/SSE.
use axum::{
    Json, Router,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::time::Duration;
pub fn card(base: &str) -> Value {
    json!({"name":"Legacy SDK fixture","description":"typed v0.3 fixture","version":"1","protocolVersion":"0.3.0","url":base,"capabilities":{"streaming":true,"pushNotifications":true},"defaultInputModes":["text/plain","application/json"],"defaultOutputModes":["text/plain"],"skills":[]})
}
pub fn router(base: &str) -> Router {
    let card = card(base);
    Router::new().route("/.well-known/agent-card.json",get(move ||{let card=card.clone();async move{Json(card)}})).route("/",post(|Json(rpc):Json<Value>|async move{
        let method=rpc["method"].as_str().unwrap();let mut context="ctx-legacy".to_owned();let mut state="completed";let id="legacy-task";
        if method=="message/send"||method=="message/stream"{let params:a2a_types_legacy::MessageSendParams=serde_json::from_value(rpc["params"].clone()).unwrap();context=params.message.context_id.clone().unwrap_or(context);if params.message.message_id=="wait"{tokio::time::sleep(Duration::from_secs(30)).await;}else if params.message.message_id=="input"{state="input-required";}}
        if method=="tasks/cancel"{state="canceled";}
        let task: a2a_types_legacy::Task=serde_json::from_value(json!({"kind":"task","id":id,"contextId":context,"status":{"state":state},"history":[{"kind":"message","messageId":"answer","role":"agent","parts":[{"kind":"text","text":"Typed SDK answer"},{"kind":"data","data":{"count":2}},{"kind":"file","file":{"uri":"file:///must-not-fetch","mimeType":"text/plain"}}]}],"artifacts":[{"artifactId":"artifact","parts":[{"kind":"text","text":"Artifact content"}]}]})).unwrap();
        if method=="message/stream"||method=="tasks/resubscribe"{let update:a2a_types_legacy::TaskStatusUpdateEvent=serde_json::from_value(json!({"kind":"status-update","taskId":id,"contextId":context,"status":{"state":"completed"},"final":true})).unwrap();let body=format!("data: {}\n\ndata: {}\n\n",json!({"jsonrpc":"2.0","id":rpc["id"],"result":task}),json!({"jsonrpc":"2.0","id":rpc["id"],"result":update}));return ([("content-type","text/event-stream")],body).into_response();}
        Json(json!({"jsonrpc":"2.0","id":rpc["id"],"result":task})).into_response()
    }))
}

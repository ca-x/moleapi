#![allow(deprecated)]
use rmcp::{RoleServer, ServerHandler, model::*, service::RequestContext};
use serde_json::{Value, json};
#[derive(Clone, Default)]
pub struct Fixture {
    pub pagination_loop: bool,
}
fn typed<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}
impl ServerHandler for Fixture {
    fn get_info(&self) -> ServerConfig {
        typed(
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":true},"resources":{"subscribe":true,"listChanged":true},"prompts":{"listChanged":true},"logging":{}},"serverInfo":{"name":"MoleAPI official SDK fixture","version":"1"}}),
        )
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = ["echo","fail","protocol_error","progress","change","sampling","elicitation","roots","slow","oversize","environment","content","private_reflection"].into_iter().map(|name| json!({"name":name,"description":format!("Fixture {name}"),"inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false}})).collect::<Vec<_>>();
        Ok(typed(
            json!({"tools":tools,"nextCursor":if self.pagination_loop {Some("repeat")} else {None}}),
        ))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let value = request.arguments.unwrap_or_default();
        let result = match request.name.as_ref() {
            "protocol_error" => {
                return Err(ErrorData::invalid_params(
                    "Fixture protocol error",
                    Some(json!({"detail":"kept"})),
                ));
            }
            "fail" => CallToolResult::structured_error(json!({"error":"Fixture tool error"})),
            "slow" => {
                context.ct.cancelled().await;
                return Err(ErrorData::internal_error("Stopped", None));
            }
            "progress" => {
                if let Some(token) = context
                    .meta
                    .get("progressToken")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                {
                    let _ = context
                        .peer
                        .notify_progress(ProgressNotificationParam::new(token, 0.5))
                        .await;
                }
                CallToolResult::structured(json!(value))
            }
            "change" => {
                let _ = context.peer.notify_tool_list_changed().await;
                let _ = context.peer.notify_prompt_list_changed().await;
                let _ = context.peer.notify_resource_list_changed().await;
                CallToolResult::structured(json!(value))
            }
            "sampling" => {
                let response=context.peer.create_message(typed(json!({"messages":[{"role":"user","content":{"type":"text","text":"Manual sampling fixture"}}],"maxTokens":20}))).await.map_err(|_|ErrorData::internal_error("Sampling declined",None))?;
                CallToolResult::structured(serde_json::to_value(response).unwrap())
            }
            "elicitation" => {
                let response=context.peer.create_elicitation(typed(json!({"message":"Manual fixture approval","requestedSchema":{"type":"object","properties":{"answer":{"type":"string"}},"required":["answer"]}}))).await.map_err(|_|ErrorData::internal_error("Elicitation failed",None))?;
                CallToolResult::structured(serde_json::to_value(response).unwrap())
            }
            "roots" => {
                let response = context
                    .peer
                    .list_roots()
                    .await
                    .map_err(|_| ErrorData::internal_error("Roots declined", None))?;
                CallToolResult::structured(serde_json::to_value(response).unwrap())
            }
            "private_reflection" => {
                let text = value
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let _ = context
                    .peer
                    .send_notification(ServerNotification::CustomNotification(
                        CustomNotification::new(
                            format!("private/{text}"),
                            Some(json!({"field":173,"flag":true})),
                        ),
                    ))
                    .await;
                CallToolResult::structured(
                    json!({text:{"number":173,"boolean":true},format!("prefix/{text}"):"reflected key"}),
                )
            }
            "content" => typed(
                json!({"content":[{"type":"text","text":"<script>never execute</script>"},{"type":"image","mimeType":"image/png","data":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aTuoAAAAASUVORK5CYII="},{"type":"audio","mimeType":"audio/wav","data":"UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAIA+AAACABAAZGF0YQAAAAA="},{"type":"resource_link","uri":"https://example.invalid/never-fetch","name":"Explicit link"},{"type":"resource","resource":{"uri":"fixture://embedded","mimeType":"application/json","text":"{\"ok\":true}"}}],"structuredContent":{"ok":true}}),
            ),
            "oversize" => CallToolResult::structured(json!({"data":"x".repeat(2*1024*1024)})),
            "environment" => CallToolResult::structured(
                json!({"explicit":std::env::var("MCP_FIXTURE_VALUE").ok(),"ambient":std::env::var("MOLEAPI_MCP_AMBIENT_CANARY").ok(),"home":std::env::var("HOME").ok(),"pid":std::process::id()}),
            ),
            _ => CallToolResult::structured(json!(value)),
        };
        Ok(result.into())
    }
    async fn list_resources(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(typed(
            json!({"resources":[{"uri":"fixture://hello","name":"Fixture resource","mimeType":"text/plain"}]}),
        ))
    }
    async fn list_resource_templates(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(typed(
            json!({"resourceTemplates":[{"uriTemplate":"fixture://item/{id}","name":"Fixture template"}]}),
        ))
    }
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        Ok(typed::<ReadResourceResult>(json!({"contents":[{"uri":request.uri,"mimeType":"text/plain","text":"SDK resource content"}]})).into())
    }
    async fn list_prompts(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        Ok(typed(
            json!({"prompts":[{"name":"greeting","arguments":[{"name":"who","required":true}]}]}),
        ))
    }
    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, ErrorData> {
        Ok(typed::<GetPromptResult>(json!({"description":"Fixture prompt","messages":[{"role":"user","content":{"type":"text","text":format!("Hello {}",request.arguments.unwrap_or_default().get("who").and_then(Value::as_str).unwrap_or("guest"))}}]})).into())
    }
    async fn subscribe(
        &self,
        request: SubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        let _ = context
            .peer
            .notify_resource_updated(typed(json!({"uri":request.uri})))
            .await;
        Ok(())
    }
    async fn unsubscribe(
        &self,
        _: UnsubscribeRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        Ok(())
    }
}
pub fn router(loop_pages: bool) -> axum::Router {
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };
    let service = StreamableHttpService::new(
        move || {
            Ok(Fixture {
                pagination_loop: loop_pages,
            })
        },
        std::sync::Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    axum::Router::new().nest_service("/mcp", service)
}

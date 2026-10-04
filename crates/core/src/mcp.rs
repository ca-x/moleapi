use crate::{Pair, Protocol, RequestSpec};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct McpConfig {
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<Pair>,
    pub operation: String,
    pub name: String,
    pub arguments_source: String,
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_source: Option<String>,
}
impl Default for McpConfig {
    fn default() -> Self {
        Self {
            transport: "http".into(),
            command: String::new(),
            args: vec![],
            env: vec![],
            operation: "tools/call".into(),
            name: String::new(),
            arguments_source: "{}".into(),
            uri: String::new(),
            config_source: None,
        }
    }
}
pub fn validate_mcp(request: &RequestSpec, draft: bool) -> Result<()> {
    let Protocol::Mcp { config } = &request.protocol else {
        return Ok(());
    };
    ensure!(
        matches!(config.transport.as_str(), "http" | "stdio"),
        "Unsupported MCP transport"
    );
    ensure!(
        config.command.len() <= 4096
            && config.args.len() <= 128
            && config.args.iter().map(String::len).sum::<usize>() <= 65536
            && config.env.len() <= 128
            && config
                .env
                .iter()
                .map(|v| v.key.len()
                    + v.value.len()
                    + v.local_value.as_ref().map_or(0, String::len))
                .sum::<usize>()
                <= 65536,
        "MCP launch configuration exceeds limit"
    );
    ensure!(
        config.arguments_source.len() <= 1024 * 1024
            && config.name.len() <= 4096
            && config.uri.len() <= 8192
            && config
                .config_source
                .as_ref()
                .is_none_or(|s| s.len() <= 1024 * 1024),
        "MCP draft exceeds limit"
    );
    if !draft && config.transport == "stdio" {
        ensure!(
            std::path::Path::new(&config.command).is_absolute() && !config.command.contains('\0'),
            "MCP STDIO requires an absolute executable path"
        );
        ensure!(
            config.args.iter().all(|s| !s.contains('\0'))
                && config
                    .env
                    .iter()
                    .filter(|v| v.enabled)
                    .all(|v| !v.key.is_empty()
                        && !v.key.contains(['=', '\0'])
                        && !v.value.contains('\0')),
            "Invalid MCP process argument or environment"
        );
    }
    Ok(())
}

/// Bounds structured protocol inputs independently of serde's recursion guard.
pub fn validate_mcp_json(value: &serde_json::Value) -> Result<()> {
    let mut nodes = 0usize;
    let mut stack = vec![(value, 0usize)];
    while let Some((value, depth)) = stack.pop() {
        nodes += 1;
        ensure!(
            nodes <= 10000 && depth <= 32,
            "MCP JSON structure exceeds limit"
        );
        match value {
            serde_json::Value::Array(items) => stack.extend(items.iter().map(|v| (v, depth + 1))),
            serde_json::Value::Object(items) => {
                stack.extend(items.values().map(|v| (v, depth + 1)))
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structured_input_depth_and_nodes_are_bounded() {
        let mut value = serde_json::json!(null);
        for _ in 0..33 {
            value = serde_json::json!([value]);
        }
        assert!(validate_mcp_json(&value).is_err());
        assert!(validate_mcp_json(&serde_json::json!(vec![0; 10001])).is_err());
    }
}

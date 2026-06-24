//! CoralOS MCP client — lets Rust agents join CoralOS sessions as first-class
//! MCP participants, identical to the Python coral_agent.py approach.
//!
//! # Usage
//! ```no_run
//! use agent_core::coral_mcp::{CoralMcpSession, CoralMention};
//!
//! #[tokio::main]
//! async fn main() {
//!     let session = CoralMcpSession::connect("http://localhost:5555/mcp", "my-agent")
//!         .await
//!         .expect("connect failed");
//!
//!     session.run_loop(|mention| async move {
//!         format!("echo: {}", mention.text)
//!     }).await;
//! }
//! ```

use anyhow::{Context, Result};
use rmcp::{
    model::CallToolRequestParams,
    serve_client,
    transport::streamable_http_client::StreamableHttpClientTransport,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Parsed fields extracted from a `coral_wait_for_mentions` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoralMention {
    /// CoralOS thread ID, if present in the response.
    pub thread_id: Option<String>,
    /// Name or ID of the agent that sent the message.
    pub sender: Option<String>,
    /// Raw text of the full response payload.
    pub text: String,
}

/// A connected CoralOS MCP session.
///
/// Connect once with [`CoralMcpSession::connect`], then call
/// [`wait_for_mention`](CoralMcpSession::wait_for_mention) and
/// [`send_message`](CoralMcpSession::send_message) in a loop, or use the
/// higher-level [`run_loop`](CoralMcpSession::run_loop).
pub struct CoralMcpSession {
    /// The running MCP client (derefs to `Peer<RoleClient>`).
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
    /// Resolved name of the wait tool (coral_wait_for_mentions or similar).
    wait_tool: String,
    /// Resolved name of the send tool (coral_send_message or similar).
    send_tool: String,
}

impl CoralMcpSession {
    /// Connect to a CoralOS MCP endpoint via the Streamable-HTTP transport.
    ///
    /// `url` is the value of `CORAL_CONNECTION_URL` injected by CoralOS.
    /// `agent_name` is used as the MCP client identity.
    pub async fn connect(url: &str, agent_name: &str) -> Result<Self> {
        // Build a reqwest-backed streamable-HTTP transport.
        // `from_uri` is an inherent method on `StreamableHttpClientTransport<reqwest::Client>`
        // from the `transport-streamable-http-client-reqwest` feature — no need to name
        // the reqwest version explicitly.
        let transport = StreamableHttpClientTransport::from_uri(url);

        // Perform MCP handshake with a no-op client handler (`()`).
        // `()` implements `ClientHandler` (and therefore `Service<RoleClient>`).
        tracing::info!("CoralOS MCP connecting as '{}' to {}", agent_name, url);
        let client = serve_client((), transport)
            .await
            .context("MCP handshake with CoralOS failed")?;

        // Discover available tools.
        let tools_result = client
            .list_tools(None)
            .await
            .context("list_tools failed")?;

        let names: Vec<String> = tools_result
            .tools
            .iter()
            .map(|t| t.name.as_ref().to_owned())
            .collect();
        tracing::info!("CoralOS MCP tools: {:?}", names);

        let wait_tool = names
            .iter()
            .find(|n| n.contains("wait_for_mention"))
            .cloned()
            .unwrap_or_else(|| "coral_wait_for_mentions".to_string());

        let send_tool = names
            .iter()
            .find(|n| n.ends_with("send_message"))
            .cloned()
            .unwrap_or_else(|| "coral_send_message".to_string());

        tracing::info!(
            "CoralOS resolved tools: wait='{}' send='{}'",
            wait_tool,
            send_tool
        );

        Ok(Self {
            client,
            wait_tool,
            send_tool,
        })
    }

    /// Block until another agent mentions us (up to `max_wait_ms` ms).
    ///
    /// Returns `None` on timeout / empty response, `Some(mention)` otherwise.
    pub async fn wait_for_mention(&self, max_wait_ms: u64) -> Result<Option<CoralMention>> {
        let args = serde_json::json!({ "maxWaitMs": max_wait_ms });
        let args_obj = args
            .as_object()
            .cloned()
            .expect("json object is always an object");

        let params = CallToolRequestParams::new(self.wait_tool.clone())
            .with_arguments(args_obj);

        let result = self
            .client
            .call_tool(params)
            .await
            .context("wait_for_mention call failed")?;

        // Collect all text content blocks.
        let text: String = result
            .content
            .iter()
            .filter_map(|c| {
                // `Content` is `Annotated<RawContent>`; the inner value is in `.raw`.
                if let rmcp::model::RawContent::Text(t) = &c.raw {
                    Some(t.text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed == "null" || trimmed == "{}" {
            return Ok(None);
        }

        Ok(Some(parse_mention(&text)))
    }

    /// Send a message into a CoralOS thread.
    ///
    /// - `content` — message body
    /// - `thread_id` — optional thread to reply into
    /// - `mentions` — list of agent names to @-mention
    pub async fn send_message(
        &self,
        content: &str,
        thread_id: Option<&str>,
        mentions: &[&str],
    ) -> Result<()> {
        let mut args = serde_json::Map::new();
        args.insert("content".into(), json!(content));
        if let Some(tid) = thread_id {
            args.insert("threadId".into(), json!(tid));
        }
        if !mentions.is_empty() {
            args.insert("mentions".into(), json!(mentions));
        }

        let params = CallToolRequestParams::new(self.send_tool.clone())
            .with_arguments(args);

        self.client
            .call_tool(params)
            .await
            .context("send_message call failed")?;

        Ok(())
    }

    /// Run the standard CoralOS agent loop until cancelled:
    ///   1. `wait_for_mention(30 s)`
    ///   2. Call `handler(mention)` → get response string
    ///   3. `send_message(response, thread_id, [sender])`
    ///
    /// On timeout the loop keeps going. On transport error it retries after 2 s.
    pub async fn run_loop<F, Fut>(&self, handler: F)
    where
        F: Fn(CoralMention) -> Fut,
        Fut: std::future::Future<Output = String>,
    {
        loop {
            match self.wait_for_mention(30_000).await {
                Ok(Some(mention)) => {
                    tracing::info!(
                        "coral mention from {:?} in thread {:?}: {}",
                        mention.sender,
                        mention.thread_id,
                        &mention.text[..mention.text.len().min(120)]
                    );
                    let response = handler(mention.clone()).await;
                    let sender_slice: Vec<&str> =
                        mention.sender.as_deref().map(|s| vec![s]).unwrap_or_default();
                    if let Err(e) = self
                        .send_message(&response, mention.thread_id.as_deref(), &sender_slice)
                        .await
                    {
                        tracing::error!("coral send_message error: {}", e);
                    }
                }
                Ok(None) => {
                    // Normal timeout — keep polling
                    tracing::debug!("coral wait_for_mention: timeout, re-polling");
                }
                Err(e) => {
                    tracing::error!(
                        "coral wait_for_mention error: {} — retrying in 2 s",
                        e
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    }
}

/// Parse the JSON blob returned by `coral_wait_for_mentions` into structured fields.
///
/// Handles all known CoralOS message shapes defensively — if the JSON shape
/// changes the raw text is still preserved in `CoralMention::text`.
fn parse_mention(text: &str) -> CoralMention {
    let mut thread_id: Option<String> = None;
    let mut sender: Option<String> = None;

    if let Ok(data) = serde_json::from_str::<serde_json::Value>(text) {
        thread_id = data
            .get("threadId")
            .or_else(|| data.get("thread_id"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        sender = data
            .get("senderName")
            .or_else(|| data.get("sender"))
            .or_else(|| data.get("senderId"))
            .or_else(|| data.get("from"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        // CoralOS sometimes wraps messages in an array under "messages"
        if let Some(msgs) = data.get("messages").and_then(|m| m.as_array()) {
            if let Some(m0) = msgs.first() {
                thread_id = thread_id.or_else(|| {
                    m0.get("threadId")
                        .or_else(|| m0.get("thread_id"))
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                });
                sender = sender.or_else(|| {
                    m0.get("senderName")
                        .or_else(|| m0.get("sender"))
                        .or_else(|| m0.get("senderId"))
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                });
            }
        }

        // CoralOS sometimes wraps a single message under "message"
        if let Some(m) = data.get("message").and_then(|v| v.as_object()) {
            let mv = serde_json::Value::Object(m.clone());
            thread_id = thread_id.or_else(|| {
                mv.get("threadId")
                    .or_else(|| mv.get("thread_id"))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            });
            sender = sender.or_else(|| {
                mv.get("senderName")
                    .or_else(|| mv.get("sender"))
                    .or_else(|| mv.get("senderId"))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            });
        }
    }

    CoralMention {
        thread_id,
        sender,
        text: text.to_string(),
    }
}

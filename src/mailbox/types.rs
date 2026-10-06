use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    /// A work assignment dispatched by the orchestrator.
    Task,
    /// An asynchronous status or progress update from a worker.
    Progress,
    /// A structured question/blocker from a subordinate requesting a decision.
    Ask,
    /// The response to an Ask message resolving the question.
    Reply,
    /// The completed work output from a worker.
    Result,
    /// An error or failure report.
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub id: String,
    pub from: String,
    pub to: String,
    pub msg_type: MessageType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub payload: serde_json::Value,
    pub timestamp: u64,
}

impl MessageEnvelope {
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        msg_type: MessageType,
        payload: serde_json::Value,
        correlation_id: Option<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let id = format!("msg_{}_{:06x}", now, fastrand());
        Self {
            id,
            from: from.into(),
            to: to.into(),
            msg_type,
            correlation_id,
            payload,
            timestamp: now,
        }
    }
}

fn fastrand() -> u32 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);
    nanos ^ (std::process::id() << 8)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskRequestPayload {
    pub question: String,
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyResponsePayload {
    pub choice: String,
    pub author: String,
}

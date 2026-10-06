pub mod types;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

pub use types::{AskRequestPayload, MessageEnvelope, MessageType, ReplyResponsePayload};

const MAX_MAILBOX_CAPACITY: usize = 1_000;

static GLOBAL_MAILBOX: OnceLock<MailboxManager> = OnceLock::new();

pub fn global() -> &'static MailboxManager {
    GLOBAL_MAILBOX.get_or_init(MailboxManager::new)
}

#[derive(Clone)]
pub struct MailboxManager {
    state: Arc<Mutex<MailboxState>>,
    condvar: Arc<Condvar>,
}

struct MailboxState {
    queues: HashMap<String, VecDeque<MessageEnvelope>>,
    pending_replies: HashMap<String, oneshot::Sender<MessageEnvelope>>,
}

impl Default for MailboxManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MailboxManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MailboxState {
                queues: HashMap::new(),
                pending_replies: HashMap::new(),
            })),
            condvar: Arc::new(Condvar::new()),
        }
    }

    /// Deposits a message into the recipient's queue and notifies any listeners.
    pub fn send(&self, envelope: MessageEnvelope) -> Result<String, String> {
        let id = envelope.id.clone();
        let to = envelope.to.clone();

        {
            let mut state = self.state.lock().map_err(|e| e.to_string())?;
            let queue = state.queues.entry(to).or_default();
            if queue.len() >= MAX_MAILBOX_CAPACITY {
                queue.pop_front();
            }
            queue.push_back(envelope);
        }

        // Wake up any threads waiting in passive wait
        self.condvar.notify_all();

        Ok(id)
    }

    /// Receives the next matching message for `recipient`.
    /// If `wait` is true, suspends the thread on an OS Condvar until a message arrives or timeout expires.
    pub fn recv(
        &self,
        recipient: &str,
        from_filter: Option<&str>,
        type_filter: Option<MessageType>,
        wait: bool,
        timeout: Option<Duration>,
    ) -> Result<Option<MessageEnvelope>, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let deadline = timeout.map(|t| Instant::now() + t);

        loop {
            // Check for matching message
            if let Some(queue) = state.queues.get_mut(recipient) {
                if let Some(pos) = queue.iter().position(|msg| {
                    if let Some(from) = from_filter {
                        if msg.from != from {
                            return false;
                        }
                    }
                    if let Some(expected_type) = type_filter {
                        if msg.msg_type != expected_type {
                            return false;
                        }
                    }
                    true
                }) {
                    return Ok(queue.remove(pos));
                }
            }

            if !wait {
                return Ok(None);
            }

            if let Some(deadline) = deadline {
                let now = Instant::now();
                if now >= deadline {
                    return Ok(None);
                }
                let remaining = deadline - now;
                let (new_state, timeout_result) = self
                    .condvar
                    .wait_timeout(state, remaining)
                    .map_err(|e| e.to_string())?;
                state = new_state;
                if timeout_result.timed_out() {
                    // Check one last time before returning None
                    if let Some(queue) = state.queues.get_mut(recipient) {
                        if let Some(pos) = queue.iter().position(|msg| {
                            if let Some(from) = from_filter {
                                if msg.from != from {
                                    return false;
                                }
                            }
                            if let Some(expected_type) = type_filter {
                                if msg.msg_type != expected_type {
                                    return false;
                                }
                            }
                            true
                        }) {
                            return Ok(queue.remove(pos));
                        }
                    }
                    return Ok(None);
                }
            } else {
                state = self.condvar.wait(state).map_err(|e| e.to_string())?;
            }
        }
    }

    /// Sends a question requiring a decision, and registers an RPC waiter that blocks
    /// until the orchestrator calls `reply`.
    pub fn ask(
        &self,
        envelope: MessageEnvelope,
        timeout: Option<Duration>,
    ) -> Result<MessageEnvelope, String> {
        let question_id = envelope.id.clone();
        let (reply_tx, mut reply_rx) = oneshot::channel();

        {
            let mut state = self.state.lock().map_err(|e| e.to_string())?;
            state.pending_replies.insert(question_id.clone(), reply_tx);
        }

        // Deliver the ask message to recipient's queue
        self.send(envelope)?;

        // Wait for the reply via blocking_recv
        if let Some(timeout) = timeout {
            let start = Instant::now();
            loop {
                match reply_rx.try_recv() {
                    Ok(reply) => return Ok(reply),
                    Err(oneshot::error::TryRecvError::Empty) => {
                        if start.elapsed() >= timeout {
                            let mut state = self.state.lock().map_err(|e| e.to_string())?;
                            state.pending_replies.remove(&question_id);
                            return Err(format!("timed out waiting for reply to question {question_id}"));
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(oneshot::error::TryRecvError::Closed) => {
                        let mut state = self.state.lock().map_err(|e| e.to_string())?;
                        state.pending_replies.remove(&question_id);
                        return Err("reply channel closed without answer".to_string());
                    }
                }
            }
        } else {
            reply_rx
                .blocking_recv()
                .map_err(|_| "reply channel closed without answer".to_string())
        }
    }

    /// Resolves a pending question by question_id with the selected choice.
    pub fn reply(
        &self,
        question_id: &str,
        choice: &str,
        author: &str,
    ) -> Result<MessageEnvelope, String> {
        let reply_envelope = MessageEnvelope::new(
            author,
            "subordinate",
            MessageType::Reply,
            serde_json::json!({
                "question_id": question_id,
                "choice": choice,
                "author": author,
            }),
            Some(question_id.to_string()),
        );

        let sender = {
            let mut state = self.state.lock().map_err(|e| e.to_string())?;
            state.pending_replies.remove(question_id)
        };

        if let Some(sender) = sender {
            sender
                .send(reply_envelope.clone())
                .map_err(|_| "failed to deliver reply to waiting subordinate".to_string())?;
            Ok(reply_envelope)
        } else {
            Err(format!("no pending question found with ID: {question_id}"))
        }
    }

    /// Lists messages in a queue without removing them.
    pub fn list(&self, recipient: Option<&str>) -> Result<Vec<MessageEnvelope>, String> {
        let state = self.state.lock().map_err(|e| e.to_string())?;
        if let Some(recipient) = recipient {
            Ok(state.queues.get(recipient).cloned().unwrap_or_default().into())
        } else {
            let mut all = Vec::new();
            for queue in state.queues.values() {
                all.extend(queue.iter().cloned());
            }
            Ok(all)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_and_recv() {
        let mb = MailboxManager::new();
        let msg = MessageEnvelope::new(
            "worker-1",
            "orchestrator",
            MessageType::Progress,
            serde_json::json!({"step": 1}),
            None,
        );
        mb.send(msg.clone()).unwrap();

        let received = mb
            .recv("orchestrator", None, None, false, None)
            .unwrap();
        assert!(received.is_some());
        assert_eq!(received.unwrap().id, msg.id);
    }

    #[test]
    fn test_ask_and_reply_rpc() {
        let mb = MailboxManager::new();
        let mb_clone = mb.clone();

        let question = MessageEnvelope::new(
            "worker-1",
            "orchestrator",
            MessageType::Ask,
            serde_json::json!({
                "question": "Which database?",
                "options": ["SQLite", "Postgres"]
            }),
            None,
        );
        let q_id = question.id.clone();

        let worker_handle = std::thread::spawn(move || {
            mb.ask(question, Some(Duration::from_secs(5)))
        });

        // Orquestador lee y responde
        let received_q = mb_clone
            .recv("orchestrator", None, Some(MessageType::Ask), true, Some(Duration::from_secs(2)))
            .unwrap()
            .unwrap();
        assert_eq!(received_q.id, q_id);

        let reply = mb_clone.reply(&q_id, "SQLite", "orchestrator").unwrap();
        assert_eq!(reply.correlation_id, Some(q_id));

        let worker_result = worker_handle.join().unwrap().unwrap();
        assert_eq!(worker_result.msg_type, MessageType::Reply);
        assert_eq!(worker_result.payload["choice"], "SQLite");
    }
}

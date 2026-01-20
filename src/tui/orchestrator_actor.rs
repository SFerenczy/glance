//! Actor-based orchestrator for concurrent TUI operations.
//!
//! Replaces `Arc<Mutex<Orchestrator>>` with a message-passing actor pattern
//! to enable cancellation and avoid lock contention during LLM/DB operations.

use crate::app::{InputResult, Orchestrator};
use crate::error::{GlanceError, Result};
use crate::tui::app::{ChatMessage, QueryLogEntry};
use crate::tui::ProgressMessage;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Requests sent to the orchestrator actor.
#[derive(Debug)]
pub enum OrchestratorRequest {
    /// Process user input (natural language or command).
    HandleInput {
        input: String,
        respond_to: oneshot::Sender<Result<InputResult>>,
        cancel: CancellationToken,
    },
    /// Confirm and execute a pending query.
    ConfirmQuery {
        sql: String,
        respond_to: oneshot::Sender<(Vec<ChatMessage>, Option<QueryLogEntry>)>,
        cancel: CancellationToken,
    },
    /// Cancel a pending query (synchronous, no DB/LLM call).
    CancelQuery {
        respond_to: oneshot::Sender<ChatMessage>,
    },
    /// Gracefully close the actor and its resources.
    Close,
}

/// The orchestrator actor that owns the orchestrator and processes requests.
pub struct OrchestratorActor {
    orchestrator: Orchestrator,
    receiver: mpsc::Receiver<OrchestratorRequest>,
    progress_tx: mpsc::Sender<ProgressMessage>,
}

impl OrchestratorActor {
    /// Creates a new actor and returns a handle for communication.
    pub fn spawn(
        orchestrator: Orchestrator,
        progress_tx: mpsc::Sender<ProgressMessage>,
    ) -> (OrchestratorHandle, Self) {
        let (sender, receiver) = mpsc::channel(32);

        let actor = Self {
            orchestrator,
            receiver,
            progress_tx,
        };

        let handle = OrchestratorHandle { sender };

        (handle, actor)
    }

    /// Runs the actor loop, processing requests until Close is received.
    pub async fn run(mut self) {
        while let Some(req) = self.receiver.recv().await {
            match req {
                OrchestratorRequest::HandleInput {
                    input,
                    respond_to,
                    cancel,
                } => {
                    let _ = self.progress_tx.send(ProgressMessage::LlmStarted).await;

                    tokio::select! {
                        biased;

                        _ = cancel.cancelled() => {
                            let _ = respond_to.send(Ok(InputResult::None));
                            let _ = self.progress_tx.send(ProgressMessage::Cancelled).await;
                        }
                        result = self.orchestrator.handle_input(&input) => {
                            let _ = respond_to.send(result);
                        }
                    }
                }

                OrchestratorRequest::ConfirmQuery {
                    sql,
                    respond_to,
                    cancel,
                } => {
                    let _ = self.progress_tx.send(ProgressMessage::DbStarted).await;

                    tokio::select! {
                        biased;

                        _ = cancel.cancelled() => {
                            let _ = respond_to.send((
                                vec![ChatMessage::System("Query cancelled.".to_string())],
                                None,
                            ));
                            let _ = self.progress_tx.send(ProgressMessage::Cancelled).await;
                        }
                        result = self.orchestrator.confirm_query(&sql) => {
                            let _ = respond_to.send(result);
                        }
                    }
                }

                OrchestratorRequest::CancelQuery { respond_to } => {
                    let msg = self.orchestrator.cancel_query();
                    let _ = respond_to.send(msg);
                }

                OrchestratorRequest::Close => {
                    break;
                }
            }
        }

        // Graceful shutdown: close database connection
        if let Err(e) = self.orchestrator.close().await {
            warn!("Error closing orchestrator: {}", e);
        }
    }
}

/// Handle for communicating with the orchestrator actor.
#[derive(Clone)]
pub struct OrchestratorHandle {
    sender: mpsc::Sender<OrchestratorRequest>,
}

impl OrchestratorHandle {
    /// Sends user input to the orchestrator for processing.
    pub async fn handle_input(
        &self,
        input: String,
        cancel: CancellationToken,
    ) -> Result<InputResult> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(OrchestratorRequest::HandleInput {
                input,
                respond_to: tx,
                cancel,
            })
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))?;

        rx.await
            .map_err(|_| GlanceError::internal("Orchestrator actor dropped response"))?
    }

    /// Confirms and executes a pending query.
    pub async fn confirm_query(
        &self,
        sql: String,
        cancel: CancellationToken,
    ) -> Result<(Vec<ChatMessage>, Option<QueryLogEntry>)> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(OrchestratorRequest::ConfirmQuery {
                sql,
                respond_to: tx,
                cancel,
            })
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))?;

        rx.await
            .map_err(|_| GlanceError::internal("Orchestrator actor dropped response"))
    }

    /// Cancels a pending query (synchronous operation).
    pub async fn cancel_query(&self) -> Result<ChatMessage> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(OrchestratorRequest::CancelQuery { respond_to: tx })
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor closed"))?;

        rx.await
            .map_err(|_| GlanceError::internal("Orchestrator actor dropped response"))
    }

    /// Signals the actor to close gracefully.
    pub async fn close(&self) -> Result<()> {
        self.sender
            .send(OrchestratorRequest::Close)
            .await
            .map_err(|_| GlanceError::internal("Orchestrator actor already closed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Schema;

    #[tokio::test]
    async fn test_actor_handle_input() {
        let orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let (progress_tx, _progress_rx) = mpsc::channel(32);
        let (handle, actor) = OrchestratorActor::spawn(orchestrator, progress_tx);

        // Spawn the actor
        let actor_handle = tokio::spawn(actor.run());

        // Send a request
        let token = CancellationToken::new();
        let result = handle.handle_input("".to_string(), token).await.unwrap();
        assert!(matches!(result, InputResult::None));

        // Close the actor
        handle.close().await.unwrap();
        actor_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_cancellation() {
        let orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let (progress_tx, mut progress_rx) = mpsc::channel(32);
        let (handle, actor) = OrchestratorActor::spawn(orchestrator, progress_tx);

        let actor_handle = tokio::spawn(actor.run());

        // Create a pre-cancelled token
        let token = CancellationToken::new();
        token.cancel();

        let result = handle
            .handle_input("/help".to_string(), token)
            .await
            .unwrap();

        // Should return None due to cancellation
        assert!(matches!(result, InputResult::None));

        // Should receive cancelled progress message
        let msg = progress_rx.recv().await.unwrap();
        assert!(matches!(msg, ProgressMessage::LlmStarted));
        let msg = progress_rx.recv().await.unwrap();
        assert!(matches!(msg, ProgressMessage::Cancelled));

        handle.close().await.unwrap();
        actor_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_cancel_query() {
        let orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let (progress_tx, _progress_rx) = mpsc::channel(32);
        let (handle, actor) = OrchestratorActor::spawn(orchestrator, progress_tx);

        let actor_handle = tokio::spawn(actor.run());

        let msg = handle.cancel_query().await.unwrap();
        match msg {
            ChatMessage::System(text) => assert!(text.contains("cancelled")),
            _ => panic!("Expected System message"),
        }

        handle.close().await.unwrap();
        actor_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_close() {
        let orchestrator = Orchestrator::with_mock_llm(None, Schema::default());
        let (progress_tx, _progress_rx) = mpsc::channel(32);
        let (handle, actor) = OrchestratorActor::spawn(orchestrator, progress_tx);

        let actor_handle = tokio::spawn(actor.run());

        // Close should succeed
        handle.close().await.unwrap();

        // Actor should complete
        actor_handle.await.unwrap();

        // Further requests should fail
        let token = CancellationToken::new();
        let result = handle.handle_input("test".to_string(), token).await;
        assert!(result.is_err());
    }
}

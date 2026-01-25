//! Progress reporting for the orchestrator actor.
//!
//! Provides a pure function to build progress messages from in-flight request state.
//! The actual timing is handled by `tokio::time::interval` in the actor loop -
//! this module only builds the progress message.

use std::time::Duration;

use super::orchestrator_actor::OrchestratorResponse;
use super::request_queue::InFlightRequest;

/// Builds progress messages for in-flight requests.
///
/// This struct is stateless and simply provides a method to construct
/// progress response messages from in-flight request state.
pub struct ProgressReporter {
    /// The interval at which progress is reported (for reference, not used directly).
    #[allow(dead_code)]
    interval: Duration,
}

impl ProgressReporter {
    /// Creates a new progress reporter with the specified interval.
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }

    /// Builds a progress response from an in-flight request.
    ///
    /// Returns an `OrchestratorResponse::Progress` containing the request's
    /// current phase and elapsed time.
    #[allow(dead_code)] // Will be used when progress reporting is fully integrated
    pub fn build_progress(&self, in_flight: &InFlightRequest) -> OrchestratorResponse {
        OrchestratorResponse::Progress {
            id: in_flight.id,
            phase: in_flight.phase,
            elapsed: in_flight.started_at.elapsed(),
            detail: None,
        }
    }

    /// Builds a progress response with a custom detail message.
    #[allow(dead_code)]
    pub fn build_progress_with_detail(
        &self,
        in_flight: &InFlightRequest,
        detail: String,
    ) -> OrchestratorResponse {
        OrchestratorResponse::Progress {
            id: in_flight.id,
            phase: in_flight.phase,
            elapsed: in_flight.started_at.elapsed(),
            detail: Some(detail),
        }
    }
}

impl Default for ProgressReporter {
    fn default() -> Self {
        // Default 100ms interval matches the actor's progress ticker
        Self::new(Duration::from_millis(100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::InputResult;
    use crate::tui::orchestrator_actor::{OperationPhase, RequestId};
    use std::time::Instant;

    /// Helper to create a test in-flight request.
    fn make_in_flight(id: RequestId, phase: OperationPhase) -> InFlightRequest {
        InFlightRequest {
            id,
            task: tokio::spawn(async { Ok(InputResult::None) }),
            started_at: Instant::now(),
            phase,
        }
    }

    #[tokio::test]
    async fn build_progress_includes_elapsed_time() {
        let reporter = ProgressReporter::default();
        let id = RequestId::new();
        let in_flight = make_in_flight(id, OperationPhase::Processing);

        // Wait a small amount to ensure elapsed time is > 0
        tokio::time::sleep(Duration::from_millis(10)).await;

        let response = reporter.build_progress(&in_flight);

        match response {
            OrchestratorResponse::Progress { elapsed, .. } => {
                assert!(elapsed >= Duration::from_millis(10));
            }
            _ => panic!("Expected Progress response"),
        }
    }

    #[tokio::test]
    async fn build_progress_includes_operation_phase() {
        let reporter = ProgressReporter::default();
        let id = RequestId::new();

        // Test each phase
        for phase in [
            OperationPhase::LlmRequesting,
            OperationPhase::LlmThinking,
            OperationPhase::LlmStreaming,
            OperationPhase::DbExecuting,
            OperationPhase::Processing,
        ] {
            let in_flight = make_in_flight(id, phase);
            let response = reporter.build_progress(&in_flight);

            match response {
                OrchestratorResponse::Progress {
                    phase: resp_phase, ..
                } => {
                    assert_eq!(resp_phase, phase);
                }
                _ => panic!("Expected Progress response"),
            }
        }
    }

    #[tokio::test]
    async fn build_progress_includes_request_id() {
        let reporter = ProgressReporter::default();
        let id = RequestId::new();
        let in_flight = make_in_flight(id, OperationPhase::Processing);

        let response = reporter.build_progress(&in_flight);

        match response {
            OrchestratorResponse::Progress { id: resp_id, .. } => {
                assert_eq!(resp_id, id);
            }
            _ => panic!("Expected Progress response"),
        }
    }

    #[tokio::test]
    async fn build_progress_with_detail_includes_detail() {
        let reporter = ProgressReporter::default();
        let id = RequestId::new();
        let in_flight = make_in_flight(id, OperationPhase::LlmStreaming);

        let response =
            reporter.build_progress_with_detail(&in_flight, "Processing token 42".to_string());

        match response {
            OrchestratorResponse::Progress { detail, .. } => {
                assert_eq!(detail, Some("Processing token 42".to_string()));
            }
            _ => panic!("Expected Progress response"),
        }
    }

    #[test]
    fn default_reporter_uses_100ms_interval() {
        let reporter = ProgressReporter::default();
        assert_eq!(reporter.interval, Duration::from_millis(100));
    }

    #[test]
    fn custom_interval_is_stored() {
        let reporter = ProgressReporter::new(Duration::from_secs(1));
        assert_eq!(reporter.interval, Duration::from_secs(1));
    }
}

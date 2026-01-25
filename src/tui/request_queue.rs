//! Request queue management for the orchestrator actor.
//!
//! Provides a pure state management layer for the request queue, separated from
//! the async actor loop. This enables unit testing of queue logic without
//! requiring the full actor infrastructure.

use std::collections::VecDeque;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use super::orchestrator_actor::{OperationPhase, RequestId, RequestType};

/// Maximum number of requests that can be queued (default).
pub const DEFAULT_MAX_QUEUE_DEPTH: usize = 10;

/// A request waiting in the queue.
#[derive(Debug)]
pub struct PendingRequest {
    /// Unique identifier for this request.
    pub id: RequestId,
    /// The input string (natural language, SQL, or confirmation).
    pub input: String,
    /// Type of request.
    pub request_type: RequestType,
    /// When this request was queued.
    #[allow(dead_code)] // Will be used for queue time tracking
    pub queued_at: Instant,
    /// Cancellation token for this request.
    pub cancel: CancellationToken,
}

/// An in-flight request being processed.
pub struct InFlightRequest {
    /// Unique identifier for this request.
    pub id: RequestId,
    /// The task handle for the processing operation.
    pub task: tokio::task::JoinHandle<Result<crate::app::InputResult, String>>,
    /// When processing started.
    pub started_at: Instant,
    /// Current phase of the operation.
    pub phase: OperationPhase,
}

/// Result of an enqueue operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueEvent {
    /// Request was added to the queue at the given position (1-indexed).
    Queued { position: usize },
    /// Queue is full, request was rejected.
    QueueFull,
}

/// Manages the request queue for the orchestrator actor.
///
/// This struct handles all queue state management including:
/// - FIFO request queuing
/// - In-flight request tracking
/// - Cancellation management
/// - Confirmation state (pauses queue processing)
pub struct RequestQueue {
    /// Pending requests waiting to be processed (FIFO).
    queue: VecDeque<PendingRequest>,
    /// Currently processing request.
    in_flight: Option<InFlightRequest>,
    /// Cancellation token for the current in-flight request.
    current_cancel: Option<CancellationToken>,
    /// Whether we're waiting for user confirmation (pauses queue).
    awaiting_confirmation: bool,
    /// Maximum number of requests that can be queued.
    max_depth: usize,
}

impl RequestQueue {
    /// Creates a new request queue with the default max depth.
    pub fn new() -> Self {
        Self::with_max_depth(DEFAULT_MAX_QUEUE_DEPTH)
    }

    /// Creates a new request queue with a custom max depth.
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            in_flight: None,
            current_cancel: None,
            awaiting_confirmation: false,
            max_depth,
        }
    }

    /// Returns the maximum queue depth.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Attempts to enqueue a request.
    ///
    /// Returns `QueueEvent::Queued` with the position if successful,
    /// or `QueueEvent::QueueFull` if the queue is at capacity.
    pub fn enqueue(&mut self, request: PendingRequest) -> QueueEvent {
        if self.queue.len() >= self.max_depth {
            return QueueEvent::QueueFull;
        }

        self.queue.push_back(request);
        QueueEvent::Queued {
            position: self.queue.len(),
        }
    }

    /// Attempts to dequeue the next request.
    ///
    /// Returns `None` if the queue is empty or if awaiting confirmation.
    pub fn try_dequeue(&mut self) -> Option<PendingRequest> {
        if self.awaiting_confirmation {
            return None;
        }
        self.queue.pop_front()
    }

    /// Returns whether the queue can process the next request.
    ///
    /// This is true when:
    /// - There is no in-flight request
    /// - The queue is not empty
    /// - We are not awaiting user confirmation
    pub fn can_process_next(&self) -> bool {
        self.in_flight.is_none() && !self.queue.is_empty() && !self.awaiting_confirmation
    }

    /// Cancels the current in-flight request.
    ///
    /// Returns the cancellation token if there was an in-flight request.
    pub fn cancel_current(&mut self) -> Option<CancellationToken> {
        if let Some(in_flight) = self.in_flight.take() {
            in_flight.task.abort();
        }
        self.current_cancel.take()
    }

    /// Cancels a specific queued request by ID.
    ///
    /// If the request is the current in-flight request, it is cancelled.
    /// If it's in the queue, it is removed and returned.
    /// Returns `None` if the request was not found.
    pub fn cancel_by_id(&mut self, id: RequestId) -> Option<PendingRequest> {
        // Check if it's the current in-flight request
        if let Some(ref in_flight) = self.in_flight {
            if in_flight.id == id {
                self.cancel_current();
                return None; // In-flight request was cancelled but not returned
            }
        }

        // Find and remove from queue
        if let Some(pos) = self.queue.iter().position(|r| r.id == id) {
            let request = self.queue.remove(pos)?;
            request.cancel.cancel();
            return Some(request);
        }

        None
    }

    /// Cancels all operations (current + queued).
    ///
    /// Returns all cancelled pending requests.
    pub fn cancel_all(&mut self) -> Vec<PendingRequest> {
        // Cancel current
        self.cancel_current();

        // Cancel and collect all queued
        let mut cancelled = Vec::new();
        while let Some(request) = self.queue.pop_front() {
            request.cancel.cancel();
            cancelled.push(request);
        }

        cancelled
    }

    /// Returns whether the queue is idle (no in-flight request and empty queue).
    #[allow(dead_code)] // Useful for testing and future use
    pub fn is_idle(&self) -> bool {
        self.in_flight.is_none() && self.queue.is_empty()
    }

    /// Sets the in-flight request and its cancellation token.
    #[allow(dead_code)] // Will be used for background task spawning
    pub fn set_in_flight(&mut self, request: InFlightRequest, cancel: CancellationToken) {
        self.in_flight = Some(request);
        self.current_cancel = Some(cancel);
    }

    /// Clears the in-flight request and returns it.
    pub fn clear_in_flight(&mut self) -> Option<InFlightRequest> {
        self.current_cancel = None;
        self.in_flight.take()
    }

    /// Sets the confirmation pending state.
    ///
    /// When true, the queue will not dequeue new requests until confirmation
    /// is received or cancelled.
    pub fn set_confirmation_pending(&mut self, pending: bool) {
        self.awaiting_confirmation = pending;
    }

    /// Returns whether the queue is awaiting confirmation.
    #[allow(dead_code)] // Useful for testing and status queries
    pub fn is_awaiting_confirmation(&self) -> bool {
        self.awaiting_confirmation
    }

    /// Returns the number of pending requests in the queue.
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Returns the queue positions for all pending requests.
    ///
    /// Returns a vector of (RequestId, position) tuples, where position is 1-indexed.
    pub fn get_queue_positions(&self) -> Vec<(RequestId, usize)> {
        self.queue
            .iter()
            .enumerate()
            .map(|(idx, req)| (req.id, idx + 1))
            .collect()
    }

    /// Returns a reference to the current in-flight request, if any.
    pub fn in_flight(&self) -> Option<&InFlightRequest> {
        self.in_flight.as_ref()
    }

    /// Returns the ID of the current in-flight request, if any.
    #[allow(dead_code)] // Useful for testing and status queries
    pub fn current_id(&self) -> Option<RequestId> {
        self.in_flight.as_ref().map(|r| r.id)
    }
}

impl Default for RequestQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test pending request.
    fn make_request(id: RequestId) -> PendingRequest {
        PendingRequest {
            id,
            input: format!("test input for {}", id),
            request_type: RequestType::NaturalLanguage,
            queued_at: Instant::now(),
            cancel: CancellationToken::new(),
        }
    }

    #[test]
    fn should_dequeue_in_fifo_order() {
        let mut queue = RequestQueue::new();
        let id_a = RequestId::new();
        let id_b = RequestId::new();
        let id_c = RequestId::new();

        queue.enqueue(make_request(id_a));
        queue.enqueue(make_request(id_b));
        queue.enqueue(make_request(id_c));

        assert_eq!(queue.try_dequeue().unwrap().id, id_a);
        assert_eq!(queue.try_dequeue().unwrap().id, id_b);
        assert_eq!(queue.try_dequeue().unwrap().id, id_c);
        assert!(queue.try_dequeue().is_none());
    }

    #[test]
    fn should_not_dequeue_while_awaiting_confirmation() {
        let mut queue = RequestQueue::new();
        let id = RequestId::new();

        queue.enqueue(make_request(id));
        queue.set_confirmation_pending(true);

        assert!(queue.try_dequeue().is_none());
        assert!(!queue.can_process_next());

        queue.set_confirmation_pending(false);
        assert!(queue.try_dequeue().is_some());
    }

    #[tokio::test]
    async fn should_cancel_in_flight_request() {
        let mut queue = RequestQueue::new();
        let id = RequestId::new();
        let cancel = CancellationToken::new();

        // Create a dummy task
        let task = tokio::spawn(async { Ok(crate::app::InputResult::None) });

        let in_flight = InFlightRequest {
            id,
            task,
            started_at: Instant::now(),
            phase: OperationPhase::Processing,
        };

        queue.set_in_flight(in_flight, cancel.clone());
        assert!(!queue.is_idle());

        let token = queue.cancel_current();
        assert!(token.is_some());
        assert!(queue.is_idle());
    }

    #[test]
    fn can_process_next_respects_confirmation_state() {
        let mut queue = RequestQueue::new();
        let id = RequestId::new();

        queue.enqueue(make_request(id));
        assert!(queue.can_process_next());

        queue.set_confirmation_pending(true);
        assert!(!queue.can_process_next());

        queue.set_confirmation_pending(false);
        assert!(queue.can_process_next());
    }

    #[test]
    fn should_reject_when_queue_full() {
        let mut queue = RequestQueue::with_max_depth(2);

        let result_a = queue.enqueue(make_request(RequestId::new()));
        assert!(matches!(result_a, QueueEvent::Queued { position: 1 }));

        let result_b = queue.enqueue(make_request(RequestId::new()));
        assert!(matches!(result_b, QueueEvent::Queued { position: 2 }));

        let result_c = queue.enqueue(make_request(RequestId::new()));
        assert!(matches!(result_c, QueueEvent::QueueFull));
    }

    #[test]
    fn cancel_by_id_removes_correct_request() {
        let mut queue = RequestQueue::new();
        let id_a = RequestId::new();
        let id_b = RequestId::new();
        let id_c = RequestId::new();

        queue.enqueue(make_request(id_a));
        queue.enqueue(make_request(id_b));
        queue.enqueue(make_request(id_c));

        // Cancel the middle one
        let cancelled = queue.cancel_by_id(id_b);
        assert!(cancelled.is_some());
        assert_eq!(cancelled.unwrap().id, id_b);

        // Should have 2 remaining in order
        assert_eq!(queue.pending_count(), 2);
        assert_eq!(queue.try_dequeue().unwrap().id, id_a);
        assert_eq!(queue.try_dequeue().unwrap().id, id_c);
    }

    #[test]
    fn cancel_by_id_returns_none_for_unknown_id() {
        let mut queue = RequestQueue::new();
        queue.enqueue(make_request(RequestId::new()));

        let unknown_id = RequestId::new();
        let result = queue.cancel_by_id(unknown_id);
        assert!(result.is_none());

        // Queue should be unchanged
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn cancel_all_clears_queue_and_in_flight() {
        let mut queue = RequestQueue::new();

        queue.enqueue(make_request(RequestId::new()));
        queue.enqueue(make_request(RequestId::new()));
        queue.enqueue(make_request(RequestId::new()));

        let cancelled = queue.cancel_all();
        assert_eq!(cancelled.len(), 3);
        assert!(queue.is_idle());
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn is_idle_when_empty_and_no_in_flight() {
        let queue = RequestQueue::new();
        assert!(queue.is_idle());
    }

    #[tokio::test]
    async fn is_not_idle_with_in_flight() {
        let mut queue = RequestQueue::new();
        let task = tokio::spawn(async { Ok(crate::app::InputResult::None) });

        let in_flight = InFlightRequest {
            id: RequestId::new(),
            task,
            started_at: Instant::now(),
            phase: OperationPhase::Processing,
        };

        queue.set_in_flight(in_flight, CancellationToken::new());
        assert!(!queue.is_idle());
    }

    #[test]
    fn is_not_idle_with_pending_requests() {
        let mut queue = RequestQueue::new();
        queue.enqueue(make_request(RequestId::new()));
        assert!(!queue.is_idle());
    }

    #[test]
    fn get_queue_positions_returns_correct_indices() {
        let mut queue = RequestQueue::new();
        let id_a = RequestId::new();
        let id_b = RequestId::new();
        let id_c = RequestId::new();

        queue.enqueue(make_request(id_a));
        queue.enqueue(make_request(id_b));
        queue.enqueue(make_request(id_c));

        let positions = queue.get_queue_positions();
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], (id_a, 1));
        assert_eq!(positions[1], (id_b, 2));
        assert_eq!(positions[2], (id_c, 3));
    }

    #[test]
    fn enqueue_returns_correct_position() {
        let mut queue = RequestQueue::new();

        let result1 = queue.enqueue(make_request(RequestId::new()));
        assert_eq!(result1, QueueEvent::Queued { position: 1 });

        let result2 = queue.enqueue(make_request(RequestId::new()));
        assert_eq!(result2, QueueEvent::Queued { position: 2 });

        // Dequeue one
        queue.try_dequeue();

        let result3 = queue.enqueue(make_request(RequestId::new()));
        assert_eq!(result3, QueueEvent::Queued { position: 2 });
    }

    #[tokio::test]
    async fn clear_in_flight_returns_the_request() {
        let mut queue = RequestQueue::new();
        let id = RequestId::new();
        let task = tokio::spawn(async { Ok(crate::app::InputResult::None) });

        let in_flight = InFlightRequest {
            id,
            task,
            started_at: Instant::now(),
            phase: OperationPhase::Processing,
        };

        queue.set_in_flight(in_flight, CancellationToken::new());
        let cleared = queue.clear_in_flight();
        assert!(cleared.is_some());
        assert_eq!(cleared.unwrap().id, id);
        assert!(queue.is_idle());
    }

    #[test]
    fn default_creates_queue_with_default_max_depth() {
        let queue = RequestQueue::default();
        assert_eq!(queue.max_depth(), DEFAULT_MAX_QUEUE_DEPTH);
    }
}

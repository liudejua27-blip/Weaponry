use std::{
    cmp::Ordering,
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use forgecad_app_server_protocol::{
    AckParams, AppServerCursor, ReplayResult, RpcError, ServerNotification, CURSOR_RESYNC_REQUIRED,
    INPUT_TOO_LARGE, JSONRPC_VERSION, MALFORMED_UPSTREAM_EVENT, SLOW_CONSUMER,
};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::canonical::{canonical_json, sha256_hex};

#[derive(Clone)]
pub struct EventQueue {
    sender: mpsc::Sender<String>,
    state: Arc<Mutex<EventQueueState>>,
    max_replay_events: usize,
    max_replay_bytes: usize,
    max_notification_bytes: usize,
}

struct ReplayEntry {
    notification: ServerNotification,
    frame_bytes: usize,
}

#[derive(Default)]
struct EventQueueState {
    replay: VecDeque<ReplayEntry>,
    replay_bytes: usize,
    acknowledged: HashMap<String, AppServerCursor>,
    evicted_through: HashMap<String, AppServerCursor>,
    last_published: HashMap<String, AppServerCursor>,
}

impl EventQueue {
    pub fn new(
        delivery_capacity: usize,
        max_replay_events: usize,
        max_replay_bytes: usize,
        max_notification_bytes: usize,
    ) -> (Self, mpsc::Receiver<String>) {
        // One control slot is reserved for stream/resyncRequired so a slow
        // consumer can be told that data delivery stopped instead of only
        // producing a background log message.
        let (sender, receiver) = mpsc::channel(delivery_capacity.max(1).saturating_add(1));
        (
            Self {
                sender,
                state: Arc::new(Mutex::new(EventQueueState::default())),
                max_replay_events: max_replay_events.max(1),
                max_replay_bytes: max_replay_bytes.max(1),
                max_notification_bytes: max_notification_bytes.max(1),
            },
            receiver,
        )
    }

    pub fn publish(
        &self,
        method: impl Into<String>,
        cursor: AppServerCursor,
        params: Value,
    ) -> Result<ServerNotification, RpcError> {
        cursor.validate()?;
        let method = method.into();
        if method.is_empty() {
            return Err(RpcError::invalid_params(
                "Notification method cannot be empty.",
            ));
        }
        let cursor_token = cursor.encode()?;
        let identity = canonical_json(&serde_json::json!({
            "method": method,
            "cursor": cursor_token,
            "params": params,
        }));
        let notification_id = format!("notification_{}", sha256_hex(identity.as_bytes()));
        let notification =
            ServerNotification::new(method, params, notification_id.clone(), cursor_token);
        let frame = serde_json::to_string(&notification).map_err(|error| {
            RpcError::internal(format!("Notification serialization failed: {error}"))
        })?;
        let frame_bytes = frame.len();
        if frame_bytes > self.max_notification_bytes || frame_bytes > self.max_replay_bytes {
            return Err(RpcError::new(
                INPUT_TOO_LARGE,
                "NOTIFICATION_TOO_LARGE",
                "Notification exceeds the bounded event byte limit.",
                false,
            ));
        }

        let mut state = self.state.lock().expect("event queue mutex poisoned");
        if let Some(existing) = state
            .replay
            .iter()
            .find(|entry| entry.notification.notification_id.as_deref() == Some(&notification_id))
        {
            return Ok(existing.notification.clone());
        }
        if let Some(last) = state.last_published.get(&cursor.thread_id) {
            if cursor.compare_position(last) != Some(Ordering::Greater) {
                return Err(RpcError::new(
                    MALFORMED_UPSTREAM_EVENT,
                    "NON_MONOTONIC_NOTIFICATION",
                    "Notification cursor must advance monotonically within its thread.",
                    false,
                ));
            }
        }
        if self.sender.capacity() <= 1 {
            return Err(slow_consumer_error());
        }
        let permit = self
            .sender
            .try_reserve()
            .map_err(|_| slow_consumer_error())?;

        while state.replay.len() >= self.max_replay_events
            || state.replay_bytes.saturating_add(frame_bytes) > self.max_replay_bytes
        {
            let can_evict = state.replay.front().is_some_and(|front| {
                let cursor = notification_cursor(&front.notification).ok();
                cursor.is_some_and(|cursor| {
                    state
                        .acknowledged
                        .get(&cursor.thread_id)
                        .and_then(|ack| cursor.compare_position(ack))
                        .is_some_and(|ordering| ordering != Ordering::Greater)
                })
            });
            if !can_evict {
                return Err(slow_consumer_error());
            }
            if let Some(evicted) = state.replay.pop_front() {
                state.replay_bytes = state.replay_bytes.saturating_sub(evicted.frame_bytes);
                if let Ok(cursor) = notification_cursor(&evicted.notification) {
                    state
                        .evicted_through
                        .insert(cursor.thread_id.clone(), cursor);
                }
            }
        }

        state
            .last_published
            .insert(cursor.thread_id.clone(), cursor);
        state.replay_bytes += frame_bytes;
        state.replay.push_back(ReplayEntry {
            notification: notification.clone(),
            frame_bytes,
        });
        drop(state);

        permit.send(frame);
        Ok(notification)
    }

    /// Delivers a bounded compatibility notification that has no durable
    /// business cursor. These frames preserve finite SSE markers and legacy
    /// job events, but are deliberately excluded from replay and ack state so
    /// they cannot invent a second ordering truth beside persisted AgentItem
    /// sequence.
    pub fn publish_transient(
        &self,
        method: impl Into<String>,
        params: Value,
    ) -> Result<ServerNotification, RpcError> {
        let method = method.into();
        if method.is_empty() {
            return Err(RpcError::invalid_params(
                "Notification method cannot be empty.",
            ));
        }
        let notification = ServerNotification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method,
            params: Some(params),
            notification_id: None,
            cursor: None,
        };
        let frame = serde_json::to_string(&notification).map_err(|error| {
            RpcError::internal(format!("Notification serialization failed: {error}"))
        })?;
        let frame_bytes = frame.len();
        if frame_bytes > self.max_notification_bytes || frame_bytes > self.max_replay_bytes {
            return Err(RpcError::new(
                INPUT_TOO_LARGE,
                "NOTIFICATION_TOO_LARGE",
                "Notification exceeds the bounded event byte limit.",
                false,
            ));
        }
        let state = self.state.lock().expect("event queue mutex poisoned");
        if self.sender.capacity() <= 1 {
            return Err(slow_consumer_error());
        }
        let permit = self
            .sender
            .try_reserve()
            .map_err(|_| slow_consumer_error())?;
        drop(state);
        permit.send(frame);
        Ok(notification)
    }

    pub fn publish_resync_required(
        &self,
        reason: impl Into<String>,
    ) -> Result<ServerNotification, RpcError> {
        let notification = ServerNotification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "stream/resyncRequired".to_string(),
            params: Some(serde_json::json!({
                "schema_version": "ForgeCADResyncRequired@1",
                "reason": reason.into(),
            })),
            notification_id: None,
            cursor: None,
        };
        let frame = serde_json::to_string(&notification).map_err(|error| {
            RpcError::internal(format!("Resync notification serialization failed: {error}"))
        })?;
        if frame.len() > self.max_notification_bytes || frame.len() > self.max_replay_bytes {
            return Err(RpcError::new(
                INPUT_TOO_LARGE,
                "NOTIFICATION_TOO_LARGE",
                "Resync notification exceeds the bounded event byte limit.",
                false,
            ));
        }
        let mut state = self.state.lock().expect("event queue mutex poisoned");
        let permit = self
            .sender
            .try_reserve()
            .map_err(|_| slow_consumer_error())?;
        state.replay.clear();
        state.replay_bytes = 0;
        state.acknowledged.clear();
        state.evicted_through.clear();
        state.last_published.clear();
        drop(state);
        permit.send(frame);
        Ok(notification)
    }

    pub fn acknowledge(&self, params: AckParams) -> Result<(), RpcError> {
        if params.notification_id.is_none() && params.cursor.is_none() {
            return Err(RpcError::invalid_params(
                "notification/ack requires notification_id or cursor.",
            ));
        }
        let mut state = self.state.lock().expect("event queue mutex poisoned");
        let by_id = params
            .notification_id
            .as_deref()
            .and_then(|notification_id| {
                state
                    .replay
                    .iter()
                    .find(|entry| {
                        entry.notification.notification_id.as_deref() == Some(notification_id)
                    })
                    .and_then(|entry| notification_cursor(&entry.notification).ok())
            });
        let by_token = params
            .cursor
            .as_deref()
            .map(AppServerCursor::decode)
            .transpose()?;
        let cursor = match (by_id, by_token) {
            (Some(left), Some(right)) if left != right => {
                return Err(RpcError::invalid_params(
                    "notification_id and cursor refer to different notifications.",
                ))
            }
            (Some(cursor), _) | (_, Some(cursor)) => cursor,
            (None, None) => return Err(resync_error("notification_id is no longer replayable.")),
        };
        if let Some(last) = state.last_published.get(&cursor.thread_id) {
            if cursor.compare_position(last) == Some(Ordering::Greater) {
                return Err(resync_error(
                    "Acknowledgement cursor is ahead of the server.",
                ));
            }
        } else {
            return Err(resync_error("Acknowledgement thread is unknown."));
        }
        let should_advance = state
            .acknowledged
            .get(&cursor.thread_id)
            .and_then(|current| cursor.compare_position(current))
            .is_none_or(|ordering| ordering == Ordering::Greater);
        if should_advance {
            state.acknowledged.insert(cursor.thread_id.clone(), cursor);
        }
        Ok(())
    }

    pub fn replay(&self, cursor_token: &str) -> Result<ReplayResult, RpcError> {
        let cursor = AppServerCursor::decode(cursor_token)?;
        let state = self.state.lock().expect("event queue mutex poisoned");
        let Some(last) = state.last_published.get(&cursor.thread_id) else {
            return Err(resync_error("Replay thread is unknown."));
        };
        if cursor.compare_position(last) == Some(Ordering::Greater) {
            return Err(resync_error("Replay cursor is ahead of the server."));
        }
        if let Some(evicted) = state.evicted_through.get(&cursor.thread_id) {
            if cursor.compare_position(evicted) == Some(Ordering::Less) {
                return Err(resync_error(
                    "Replay cursor predates the retained event window.",
                ));
            }
        }
        let notifications = state
            .replay
            .iter()
            .filter_map(|entry| {
                let candidate = notification_cursor(&entry.notification).ok()?;
                (candidate.thread_id == cursor.thread_id
                    && candidate.compare_position(&cursor) == Some(Ordering::Greater))
                .then(|| entry.notification.clone())
            })
            .collect();
        Ok(ReplayResult { notifications })
    }

    pub fn retained_len(&self) -> usize {
        self.state
            .lock()
            .expect("event queue mutex poisoned")
            .replay
            .len()
    }

    pub fn retained_bytes(&self) -> usize {
        self.state
            .lock()
            .expect("event queue mutex poisoned")
            .replay_bytes
    }
}

fn notification_cursor(notification: &ServerNotification) -> Result<AppServerCursor, RpcError> {
    notification
        .cursor
        .as_deref()
        .ok_or_else(|| RpcError::internal("Replay notification is missing its cursor."))
        .and_then(AppServerCursor::decode)
}

fn slow_consumer_error() -> RpcError {
    let mut error = RpcError::new(
        SLOW_CONSUMER,
        "SLOW_CONSUMER",
        "The bounded notification queue is full; acknowledge and replay before continuing.",
        true,
    );
    error.data.retry_after_ms = Some(25);
    error
}

fn resync_error(message: &str) -> RpcError {
    let mut error = RpcError::new(
        CURSOR_RESYNC_REQUIRED,
        "CURSOR_RESYNC_REQUIRED",
        message,
        true,
    );
    error
        .data
        .details
        .insert("resync_hint".into(), Value::String("thread/read".into()));
    error
}

#[cfg(test)]
mod tests {
    use forgecad_app_server_protocol::CursorPhase;
    use serde_json::json;

    use super::*;

    fn cursor(sequence: u64) -> AppServerCursor {
        AppServerCursor::new(
            "thread_1",
            Some("turn_1".into()),
            sequence,
            CursorPhase::Item,
            Some(format!("item_{sequence}")),
        )
    }

    #[test]
    fn transient_notifications_are_bounded_but_not_replayable() {
        let (queue, mut receiver) = EventQueue::new(1, 4, 4096, 4096);
        let notification = queue
            .publish_transient("compat/sse", json!({"event": "agent.replay.complete"}))
            .unwrap();
        assert!(notification.notification_id.is_none());
        assert!(notification.cursor.is_none());
        assert_eq!(queue.retained_len(), 0);
        let delivered = receiver.try_recv().unwrap();
        let decoded: ServerNotification = serde_json::from_str(&delivered).unwrap();
        assert_eq!(decoded, notification);
    }

    #[test]
    fn notification_id_is_deterministic_and_payload_key_order_independent() {
        let (queue, mut receiver) = EventQueue::new(2, 4, 4096, 4096);
        let first = queue
            .publish("item/completed", cursor(1), json!({"b": 2, "a": 1}))
            .unwrap();
        let frame = receiver.try_recv().unwrap();
        let decoded: ServerNotification = serde_json::from_str(&frame).unwrap();
        assert_eq!(first.notification_id, decoded.notification_id);
        let duplicate = queue
            .publish("item/completed", cursor(1), json!({"a": 1, "b": 2}))
            .unwrap();
        assert_eq!(first.notification_id, duplicate.notification_id);
        assert_eq!(queue.retained_len(), 1);
    }

    #[test]
    fn full_queue_reports_slow_consumer_and_reserves_resync_delivery() {
        let (queue, mut receiver) = EventQueue::new(1, 8, 600, 512);
        queue
            .publish("item/completed", cursor(1), json!({"value": "a"}))
            .unwrap();
        let error = queue
            .publish("item/completed", cursor(2), json!({"value": "b"}))
            .unwrap_err();
        assert_eq!(error.code, SLOW_CONSUMER);
        assert!(queue.retained_bytes() <= 600);
        queue.publish_resync_required("slow_consumer").unwrap();
        let first: ServerNotification =
            serde_json::from_str(&receiver.try_recv().unwrap()).unwrap();
        let resync: ServerNotification =
            serde_json::from_str(&receiver.try_recv().unwrap()).unwrap();
        assert_eq!(first.method, "item/completed");
        assert_eq!(resync.method, "stream/resyncRequired");
        assert_eq!(resync.params.unwrap()["reason"], "slow_consumer");
        assert_eq!(queue.retained_len(), 0);
        assert_eq!(queue.retained_bytes(), 0);
    }

    #[test]
    fn ack_allows_eviction_and_replay_is_exclusive() {
        let (queue, mut receiver) = EventQueue::new(4, 2, 4096, 2048);
        let first = queue
            .publish("item/completed", cursor(1), json!({"n": 1}))
            .unwrap();
        queue
            .publish("item/completed", cursor(2), json!({"n": 2}))
            .unwrap();
        while receiver.try_recv().is_ok() {}
        queue
            .acknowledge(AckParams {
                notification_id: first.notification_id,
                cursor: first.cursor.clone(),
            })
            .unwrap();
        queue
            .publish("item/completed", cursor(3), json!({"n": 3}))
            .unwrap();
        let replay = queue.replay(&cursor(1).encode().unwrap()).unwrap();
        assert_eq!(replay.notifications.len(), 2);
        assert_eq!(
            replay.notifications[0].cursor.as_deref(),
            Some(cursor(2).encode().unwrap().as_str())
        );
        assert_eq!(
            queue.replay(&cursor(0).encode().unwrap()).unwrap_err().code,
            CURSOR_RESYNC_REQUIRED
        );
    }
}

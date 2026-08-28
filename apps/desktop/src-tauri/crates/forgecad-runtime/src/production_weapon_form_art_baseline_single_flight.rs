//! In-process single-flight coordination for the expensive FormArt baseline.
//!
//! The durable Store idempotency row remains the source of truth.  This small
//! Runtime-owned index only closes the window between the durable lookup and
//! the six fixed Render Worker calls, so concurrent callers of the same
//! project/key/request cannot render the same baseline twice.  It deliberately
//! stores the exact owner response (or exact failure string), not a synthetic
//! projection.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The authenticated IPC request has a 180-second absolute deadline.  A
/// waiter uses the same bound so a missing owner cannot leave a Runtime thread
/// blocked forever; the owner guard's Drop path wakes all waiters on failure.
pub(crate) const WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_IN_FLIGHT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The serialized prepare result.  Waiters return these bytes verbatim so
    /// concurrent callers observe the exact owner result.
    Completed(String),
    /// A stable owner error.  It is intentionally not persisted as a durable
    /// idempotency row; a later call may retry after the flight is removed.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Identity {
    project_id: String,
    request_sha256: String,
}

#[derive(Debug)]
struct State {
    outcome: Option<Outcome>,
}

#[derive(Debug)]
struct Flight {
    identity: Identity,
    state: Mutex<State>,
    condition: Condvar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BeginError {
    /// The same project/key is already executing a different request hash.
    Conflict,
    /// Bound the number of concurrent expensive operations.
    Capacity,
    /// The coordination lock was poisoned by an earlier panic.
    LockPoisoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaitError {
    Timeout,
    LockPoisoned,
    MissingOutcome,
}

#[derive(Debug, Default)]
pub(crate) struct Flights {
    entries: Arc<Mutex<HashMap<String, Arc<Flight>>>>,
}

#[derive(Debug)]
pub(crate) struct Guard {
    key: String,
    flight: Arc<Flight>,
    entries: Arc<Mutex<HashMap<String, Arc<Flight>>>>,
    owner: bool,
}

impl Flights {
    pub(crate) fn begin(
        &self,
        project_id: &str,
        idempotency_key: &str,
        request_sha256: &str,
    ) -> Result<Guard, BeginError> {
        let identity = Identity {
            project_id: project_id.to_owned(),
            request_sha256: request_sha256.to_owned(),
        };
        let mut entries = self.entries.lock().map_err(|_| BeginError::LockPoisoned)?;
        if let Some(flight) = entries.get(idempotency_key).cloned() {
            if flight.identity != identity {
                return Err(BeginError::Conflict);
            }
            return Ok(Guard {
                key: idempotency_key.to_owned(),
                flight,
                entries: Arc::clone(&self.entries),
                owner: false,
            });
        }
        if entries.len() >= MAX_IN_FLIGHT {
            return Err(BeginError::Capacity);
        }
        let flight = Arc::new(Flight {
            identity,
            state: Mutex::new(State { outcome: None }),
            condition: Condvar::new(),
        });
        entries.insert(idempotency_key.to_owned(), Arc::clone(&flight));
        Ok(Guard {
            key: idempotency_key.to_owned(),
            flight,
            entries: Arc::clone(&self.entries),
            owner: true,
        })
    }

    pub(crate) fn wait(&self, guard: &Guard) -> Result<Outcome, WaitError> {
        if guard.owner {
            return Err(WaitError::MissingOutcome);
        }
        let deadline = Instant::now()
            .checked_add(WAIT_TIMEOUT)
            .unwrap_or_else(Instant::now);
        let mut state = guard
            .flight
            .state
            .lock()
            .map_err(|_| WaitError::LockPoisoned)?;
        while state.outcome.is_none() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(WaitError::Timeout);
            }
            let (next_state, wait_result) = guard
                .flight
                .condition
                .wait_timeout(state, remaining)
                .map_err(|_| WaitError::LockPoisoned)?;
            state = next_state;
            if wait_result.timed_out() && state.outcome.is_none() {
                return Err(WaitError::Timeout);
            }
        }
        state.outcome.clone().ok_or(WaitError::MissingOutcome)
    }

    pub(crate) fn complete(&self, guard: &Guard, outcome: Outcome) {
        guard.complete_best_effort(outcome);
    }
}

impl Guard {
    pub(crate) fn is_owner(&self) -> bool {
        self.owner
    }

    fn complete_best_effort(&self, outcome: Outcome) {
        if !self.owner {
            return;
        }
        if let Ok(mut state) = self.flight.state.lock() {
            if state.outcome.is_none() {
                state.outcome = Some(outcome);
                self.flight.condition.notify_all();
            }
        }
        // Remove only our exact Arc.  This prevents a late Drop from deleting
        // a newer retry which reused the same idempotency key.
        if let Ok(mut entries) = self.entries.lock() {
            if entries
                .get(&self.key)
                .is_some_and(|flight| Arc::ptr_eq(flight, &self.flight))
            {
                entries.remove(&self.key);
            }
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        // A panic or early return must never strand waiters.  A failed flight
        // is not durable and the key is retryable after this guard is removed.
        self.complete_best_effort(Outcome::Failed(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_OWNER_DROPPED".to_owned(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;
    use std::thread;

    #[test]
    fn exact_owner_result_replays_and_conflict_wakes_after_failure() {
        let flights = Arc::new(Flights::default());
        let owner = flights
            .begin("project", "key", &"a".repeat(64))
            .expect("owner");
        assert!(owner.is_owner());
        let waiter = flights
            .begin("project", "key", &"a".repeat(64))
            .expect("waiter");
        assert!(!waiter.is_owner());
        assert!(matches!(
            flights.begin("project", "key", &"b".repeat(64)),
            Err(BeginError::Conflict)
        ));

        let barrier = Arc::new(Barrier::new(2));
        let flights_for_thread = Arc::clone(&flights);
        let barrier_for_thread = Arc::clone(&barrier);
        let waiter_thread = thread::spawn(move || {
            barrier_for_thread.wait();
            flights_for_thread.wait(&waiter).expect("waiter outcome")
        });
        barrier.wait();
        flights.complete(&owner, Outcome::Completed("{\"exact\":true}".to_owned()));
        assert_eq!(
            waiter_thread.join().expect("waiter join"),
            Outcome::Completed("{\"exact\":true}".to_owned())
        );
        drop(owner);
        let retry_owner = flights
            .begin("project", "key", &"a".repeat(64))
            .expect("retry owner");
        let retry_waiter = flights
            .begin("project", "key", &"a".repeat(64))
            .expect("retry waiter");
        let retry_waiter_thread =
            thread::spawn(move || flights.wait(&retry_waiter).expect("failed waiter outcome"));
        drop(retry_owner);
        assert_eq!(
            retry_waiter_thread.join().expect("failed waiter join"),
            Outcome::Failed(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_SINGLE_FLIGHT_OWNER_DROPPED".to_owned()
            )
        );
    }
}

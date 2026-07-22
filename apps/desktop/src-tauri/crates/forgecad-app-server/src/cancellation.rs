use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    task::{Context, Poll, Waker},
};

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    inner: Arc<CancellationInner>,
}

#[derive(Debug, Default)]
struct CancellationInner {
    cancelled: AtomicBool,
    waiters: Mutex<Vec<Waker>>,
    children: Mutex<Vec<Weak<CancellationInner>>>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        if self.inner.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }
        let waiters = std::mem::take(
            &mut *self
                .inner
                .waiters
                .lock()
                .expect("cancellation waiter mutex poisoned"),
        );
        for waiter in waiters {
            waiter.wake();
        }
        let children = std::mem::take(
            &mut *self
                .inner
                .children
                .lock()
                .expect("cancellation child mutex poisoned"),
        );
        for child in children.into_iter().filter_map(|child| child.upgrade()) {
            CancellationToken { inner: child }.cancel();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    pub fn cancelled(&self) -> Cancelled {
        Cancelled {
            token: self.clone(),
        }
    }

    pub fn cancelled_owned(self) -> Cancelled {
        Cancelled { token: self }
    }

    /// Creates a child scope whose cancellation is independent until its
    /// parent is cancelled. Parent cancellation is propagated transitively so
    /// one Turn can stop Provider and Product Tool work without sharing a
    /// mutable registry of task handles.
    pub fn child_token(&self) -> Self {
        let child = Self::new();
        if self.is_cancelled() {
            child.cancel();
            return child;
        }
        self.inner
            .children
            .lock()
            .expect("cancellation child mutex poisoned")
            .push(Arc::downgrade(&child.inner));
        // Close the race where the parent was cancelled between the first
        // check and registration. cancel() is idempotent on both tokens.
        if self.is_cancelled() {
            child.cancel();
        }
        child
    }
}

pub struct Cancelled {
    token: CancellationToken,
}

impl Future for Cancelled {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.token.is_cancelled() {
            return Poll::Ready(());
        }
        let mut waiters = self
            .token
            .inner
            .waiters
            .lock()
            .expect("cancellation waiter mutex poisoned");
        if self.token.is_cancelled() {
            return Poll::Ready(());
        }
        if !waiters
            .iter()
            .any(|waiter| waiter.will_wake(context.waker()))
        {
            waiters.push(context.waker().clone());
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_idempotent_and_shared_by_clones() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert!(!clone.is_cancelled());
        token.cancel();
        token.cancel();
        assert!(clone.is_cancelled());
    }

    #[test]
    fn parent_cancellation_propagates_to_descendants_but_not_siblings_upward() {
        let parent = CancellationToken::new();
        let first = parent.child_token();
        let grandchild = first.child_token();
        let second = parent.child_token();

        first.cancel();
        assert!(first.is_cancelled());
        assert!(grandchild.is_cancelled());
        assert!(!parent.is_cancelled());
        assert!(!second.is_cancelled());

        parent.cancel();
        assert!(second.is_cancelled());
    }

    #[test]
    fn child_created_after_parent_cancellation_starts_cancelled() {
        let parent = CancellationToken::new();
        parent.cancel();
        assert!(parent.child_token().is_cancelled());
    }
}

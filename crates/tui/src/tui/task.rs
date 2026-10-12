//! Background work that ends with whatever started it.
//!
//! Results of a replaced listing, a closed column or an outdated search are
//! ignored by generation checks, but the request behind them would still run
//! to completion. A [`TaskSlot`] aborts it instead: when a newer task takes
//! the slot, when it is cancelled, and when the owner (a column, the search
//! overlay) is dropped.

use std::future::Future;

use tokio::task::AbortHandle;

#[derive(Default)]
pub struct TaskSlot(Option<AbortHandle>);

impl TaskSlot {
    /// Run `fut` on the runtime in place of the previous task, which is aborted.
    pub fn spawn<F>(&mut self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.cancel();
        self.0 = Some(tokio::spawn(fut).abort_handle());
    }

    /// Abort the running task, if any.
    pub fn cancel(&mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
        }
    }

    /// Whether a task is still running.
    pub fn active(&self) -> bool {
        self.0.as_ref().is_some_and(|t| !t.is_finished())
    }
}

impl Drop for TaskSlot {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    /// Sets a flag when the task holding it is dropped, which happens when it is aborted or finishes.
    struct Lived(Arc<AtomicBool>);
    impl Drop for Lived {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn parked() -> (impl Future<Output = ()> + Send + 'static, Arc<AtomicBool>) {
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = Lived(dropped.clone());
        (
            async move {
                let _guard = guard;
                tokio::time::sleep(Duration::from_secs(3600)).await;
            },
            dropped,
        )
    }

    async fn settle() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn a_new_task_aborts_the_previous_one() {
        let mut slot = TaskSlot::default();
        let (first, first_dropped) = parked();
        slot.spawn(first);
        settle().await;
        assert!(slot.active() && !first_dropped.load(Ordering::SeqCst));
        let (second, second_dropped) = parked();
        slot.spawn(second);
        settle().await;
        assert!(first_dropped.load(Ordering::SeqCst), "the replaced task must be aborted");
        assert!(!second_dropped.load(Ordering::SeqCst));
        slot.cancel();
        settle().await;
        assert!(second_dropped.load(Ordering::SeqCst));
        assert!(!slot.active());
    }

    #[tokio::test]
    async fn dropping_the_owner_aborts_the_task() {
        let (task, dropped) = parked();
        let mut slot = TaskSlot::default();
        slot.spawn(task);
        settle().await;
        drop(slot);
        settle().await;
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn a_finished_task_is_inactive() {
        let mut slot = TaskSlot::default();
        slot.spawn(async {});
        settle().await;
        assert!(!slot.active());
    }
}

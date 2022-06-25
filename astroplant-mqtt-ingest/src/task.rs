use std::{cell::RefCell, rc::Rc};
use std::{collections::VecDeque, future::Future};
use tokio::sync::oneshot;
use tokio::sync::Notify;

type Fut<E> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), E>> + 'static>>;
type RunnerQueue<E> = VecDeque<oneshot::Sender<Fut<E>>>;

/// A nearly-lockless local task queue to run up to N tasks simultaneously.
///
/// The task queue requires to be run inside of a [LocalSet](tokio::task::LocalSet), so it can
/// spawn non-send futures,
///
/// If there are N tasks currently running, the N+1th task will wait before being added to apply
/// backpressure.
pub(crate) struct LocalTaskPool<E> {
    runners: Rc<RefCell<RunnerQueue<E>>>,
    notify: Rc<Notify>,
}

impl<E: std::fmt::Debug + 'static> LocalTaskPool<E> {
    /// Start the task pool with `num_runners` runners.
    ///
    /// # Panics
    /// Panics if this is polled outside of a [LocalSet](tokio::task::LocalSet)
    pub(crate) fn start(num_runners: usize) -> Self {
        let runners = Rc::new(RefCell::<RunnerQueue<E>>::new(VecDeque::new()));
        let notify = Rc::new(Notify::new());

        for runner in 0..num_runners {
            // Runners is Weak, to ensure runners shut down when the task pool is dropped (after
            // they finish their tasks).
            let runners = Rc::downgrade(&runners);
            let notify = notify.clone();

            tokio::task::spawn_local(async move {
                loop {
                    tracing::trace!("runner {} ready", runner);

                    let task_rx = {
                        let (task_tx, task_rx) = oneshot::channel();
                        match runners.upgrade() {
                            None => {}
                            Some(runners) => {
                                runners.borrow_mut().push_back(task_tx);
                                notify.notify_one();
                            }
                        };
                        task_rx
                    };

                    match task_rx.await {
                        Ok(task) => {
                            if let Err(err) = task.await {
                                tracing::warn!(
                                    "Runner {}: error occurred in task {:?}",
                                    runner,
                                    err
                                );
                            }
                        }
                        // Task sender was dropped.
                        Err(_) => break,
                    };
                }

                tracing::trace!("runner {} stopped", runner);
            });
        }

        Self { runners, notify }
    }

    pub(crate) async fn enqueue<F>(&self, task: F)
    where
        F: Future<Output = Result<(), E>> + 'static,
    {
        loop {
            let task_tx = self.runners.borrow_mut().pop_front();
            if let Some(task_tx) = task_tx {
                let _ = task_tx.send(Box::pin(task));
                break;
            }

            self.notify.notified().await;
        }
    }
}

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use futures::stream::{BoxStream, Stream, StreamExt};
use tokio::sync::watch;
use tracing::{Instrument, Span};

/// Either a value, an error, or a value alongside the error that made it
/// stale.
///
/// A state is kept once obtained, even once the source starts failing: a
/// freshly failing source therefore reports the last good value alongside
/// the new error as [`Partial`], rather than losing it. [`Err`] only occurs
/// when the source has never yet succeeded.
///
/// [`Partial`]: Fallible::Partial
/// [`Err`]: Fallible::Err
#[derive(Debug, Clone)]
pub enum Fallible<T, E> {
    Ok(T),
    Err(E),
    Partial { error: E, value: T },
}

impl<T, E> Fallible<T, E> {
    /// The value, if any was ever obtained.
    pub fn value(&self) -> Option<&T> {
        match self {
            Fallible::Ok(value) | Fallible::Partial { value, .. } => Some(value),
            Fallible::Err(_) => None,
        }
    }

    /// The error, if the source is currently failing.
    pub fn error(&self) -> Option<&E> {
        match self {
            Fallible::Err(error) | Fallible::Partial { error, .. } => Some(error),
            Fallible::Ok(_) => None,
        }
    }
}

/// The state of a [`StateSource`] as last reported to listeners.
pub type Reading<T, E> = Fallible<Arc<T>, Arc<E>>;

/// Something a [`LiveState`] can fetch a state from and listen to for changes.
pub trait StateSource: Send + Sync + 'static {
    type State: Send + Sync + 'static;
    /// The error type yielded on connection or stream failure, left to the
    /// implementor since it depends on the source (e.g. `anyhow::Error` for a
    /// source with ad hoc failures, or a specific error type for one backed
    /// by a typed API).
    type Error: Send + Sync + std::fmt::Debug + 'static;

    /// Fetches the current state, returning it along with a stream of the
    /// states that follow it.
    ///
    /// An error, either from this or yielded by the stream, is handed to
    /// listeners alongside the last state that was successfully obtained (if
    /// any, see [`Reading`]), and the source is connected to again after
    /// [`Options::retry_interval`]. The stream ending is not an error: the
    /// last state is kept and the source is connected to again after the same
    /// interval. A source without change notifications can therefore return
    /// an empty stream to be polled at that interval.
    fn connect(
        &self,
    ) -> impl Future<
        Output = Result<
            (Self::State, BoxStream<'_, Result<Self::State, Self::Error>>),
            Self::Error,
        >,
    > + Send;
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// How long to wait before connecting to the source again after an error
    /// or the end of its update stream.
    pub retry_interval: Duration,
    /// How long to keep listening to the source once the last listener is
    /// gone, so that listeners coming and going in quick succession don't
    /// cause a reconnection each time.
    pub linger: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            retry_interval: Duration::from_secs(5),
            linger: Duration::from_secs(10),
        }
    }
}

/// Serves the state of a [`StateSource`] to any number of listeners.
///
/// Nothing runs while there are no listeners. The first [`subscribe`] spawns
/// a task which connects to the source and keeps the state up to date for as
/// long as there are listeners, plus [`Options::linger`]. Once that task
/// stops, the state is forgotten so the next listener never sees a stale one.
///
/// [`subscribe`]: LiveState::subscribe
#[derive(Debug)]
pub struct LiveState<S: StateSource> {
    source: Arc<S>,
    shared: Arc<Shared<S::State, S::Error>>,
    options: Options,
    span: Span,
}

impl<S: StateSource> Clone for LiveState<S> {
    fn clone(&self) -> Self {
        Self {
            source: Arc::clone(&self.source),
            shared: Arc::clone(&self.shared),
            options: self.options,
            span: self.span.clone(),
        }
    }
}

#[derive(Debug)]
struct Shared<T, E> {
    /// `None` until the task first hears from the source, and again once it
    /// stops.
    state: watch::Sender<Option<Reading<T, E>>>,
    control: watch::Sender<Control>,
}

/// Kept in a single channel so that listeners arriving and the task deciding
/// to stop are serialized by its lock.
#[derive(Debug, Default)]
struct Control {
    listeners: usize,
    running: bool,
}

impl<S: StateSource> LiveState<S> {
    pub fn new(source: S) -> Self {
        Self::with_options(source, Default::default())
    }

    pub fn with_options(source: S, options: Options) -> Self {
        Self {
            source: Arc::new(source),
            shared: Arc::new(Shared {
                state: watch::channel(None).0,
                control: watch::channel(Control::default()).0,
            }),
            options,
            span: Span::none(),
        }
    }

    /// Sets the span the task listening to the source runs in, to tell apart
    /// the logs of different instances.
    pub fn instrument(mut self, span: Span) -> Self {
        self.span = span;
        self
    }

    pub fn source(&self) -> &S {
        &self.source
    }

    /// Registers a new listener, starting to listen to the source if nothing
    /// was, and waits until a state is known.
    pub async fn subscribe(&self) -> Subscription<S::State, S::Error> {
        let mut spawn = false;
        self.shared.control.send_modify(|control| {
            control.listeners += 1;
            if !control.running {
                control.running = true;
                spawn = true;
            }
        });

        if spawn {
            tokio::spawn(
                run(
                    Arc::clone(&self.source),
                    Arc::clone(&self.shared),
                    self.options,
                )
                .instrument(self.span.clone()),
            );
        }

        // Built before waiting so that the listener is unregistered if this
        // future is dropped.
        let mut subscription = Subscription {
            shared: Arc::clone(&self.shared),
            receiver: self.shared.state.subscribe(),
        };

        subscription
            .receiver
            .wait_for(Option::is_some)
            .await
            .expect("the sender is owned by the subscription");

        subscription
    }

    /// Shorthand for subscribing only to get the current state. The source
    /// is still listened to for [`Options::linger`], so repeated calls are
    /// cheap.
    pub async fn get(&self) -> Reading<S::State, S::Error> {
        self.subscribe().await.get()
    }
}

async fn run<S: StateSource>(
    source: Arc<S>,
    shared: Arc<Shared<S::State, S::Error>>,
    options: Options,
) {
    tracing::debug!(
        source = std::any::type_name::<S>(),
        "starting to follow state"
    );

    tokio::select! {
        never = follow(&*source, &shared, options.retry_interval) => match never {},
        () = wait_until_unused(&shared, options.linger) => {}
    }

    tracing::debug!(
        source = std::any::type_name::<S>(),
        "stopped following state"
    );
}

/// Keeps `shared.state` up to date with the source, forever.
async fn follow<S: StateSource>(
    source: &S,
    shared: &Shared<S::State, S::Error>,
    retry_interval: Duration,
) -> std::convert::Infallible {
    // A new state replaces the previous one and clears any error. An error
    // is recorded alongside whatever state is already there, so listeners
    // keep the last good state instead of losing it.
    let publish_state = |state: S::State| {
        shared.state.send_modify(|reading| {
            *reading = Some(Reading::Ok(Arc::new(state)));
        });
    };

    let publish_error = |error: S::Error| {
        shared.state.send_modify(|reading| {
            let error = Arc::new(error);
            *reading = Some(
                match reading.take().and_then(|reading| reading.value().cloned()) {
                    Some(value) => Reading::Partial { error, value },
                    None => Reading::Err(error),
                },
            );
        });
    };

    loop {
        tracing::debug!("connecting to source");

        let error = match source.connect().await {
            Ok((state, mut updates)) => {
                tracing::debug!("connected to source, got initial state");
                publish_state(state);

                loop {
                    match updates.next().await {
                        Some(Ok(state)) => {
                            tracing::debug!("got state update");
                            publish_state(state);
                        }
                        Some(Err(error)) => break Some(error),
                        None => {
                            tracing::debug!("update stream ended");
                            break None;
                        }
                    }
                }
            }
            Err(error) => Some(error),
        };

        if let Some(error) = error {
            tracing::error!(
                source = std::any::type_name::<S>(),
                ?error,
                "error following state"
            );
            publish_error(error);
        }

        tracing::debug!(?retry_interval, "waiting before connecting again");
        tokio::time::sleep(retry_interval).await;
    }
}

/// Returns once there have been no listeners for `linger`, having marked the
/// task as stopped and forgotten the state.
async fn wait_until_unused<T, E>(shared: &Shared<T, E>, linger: Duration) {
    let mut control = shared.control.subscribe();

    loop {
        let _ = control.wait_for(|control| control.listeners == 0).await;
        tracing::debug!(?linger, "no listeners left, stopping after delay");

        // Any change restarts the delay, including a listener that came and
        // went before this task got to see it.
        tokio::select! {
            _ = control.changed() => {
                tracing::debug!(
                    listeners = control.borrow().listeners,
                    "listeners changed, delaying stop"
                );
                continue;
            }
            () = tokio::time::sleep(linger) => {}
        }

        let mut stopped = false;
        shared.control.send_if_modified(|control| {
            // A listener may have arrived since the timer fired.
            if control.listeners == 0 {
                control.running = false;
                shared.state.send_replace(None);
                stopped = true;
            }
            false
        });

        if stopped {
            return;
        }

        tracing::debug!("listener arrived as the delay ended, not stopping");
    }
}

/// A listener of a [`LiveState`], which always holds a [`Reading`].
#[derive(Debug)]
pub struct Subscription<T, E> {
    shared: Arc<Shared<T, E>>,
    receiver: watch::Receiver<Option<Reading<T, E>>>,
}

impl<T, E> Subscription<T, E> {
    /// Returns the latest reading, marking it as seen.
    pub fn get(&mut self) -> Reading<T, E> {
        self.receiver
            .borrow_and_update()
            .clone()
            .expect("the reading is kept while there are listeners")
    }

    /// Waits for a reading that hasn't been seen through [`get`] or
    /// [`changed`] yet and returns it. The reading held when subscribing
    /// counts as seen.
    ///
    /// [`get`]: Subscription::get
    /// [`changed`]: Subscription::changed
    pub async fn changed(&mut self) -> Reading<T, E> {
        self.receiver
            .changed()
            .await
            .expect("the sender is owned by the subscription");

        self.get()
    }

    /// Turns this into a stream of the current reading followed by every
    /// subsequent one. Readings replaced before being polled are skipped.
    pub fn into_stream(self) -> impl Stream<Item = Reading<T, E>>
    where
        T: Send + Sync,
        E: Send + Sync,
    {
        futures::stream::unfold((self, true), |(mut subscription, first)| async move {
            let reading = if first {
                subscription.get()
            } else {
                subscription.changed().await
            };

            Some((reading, (subscription, false)))
        })
    }
}

impl<T, E> Clone for Subscription<T, E> {
    fn clone(&self) -> Self {
        self.shared
            .control
            .send_modify(|control| control.listeners += 1);

        Self {
            shared: Arc::clone(&self.shared),
            receiver: self.receiver.clone(),
        }
    }
}

impl<T, E> Drop for Subscription<T, E> {
    fn drop(&mut self) {
        self.shared
            .control
            .send_modify(|control| control.listeners -= 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::anyhow;
    use tokio::sync::mpsc;

    /// Fails `failures` times, then yields the number of connections followed
    /// by the values sent through `updates`, once.
    struct TestSource {
        connections: AtomicUsize,
        failures: usize,
        updates: tokio::sync::Mutex<Option<mpsc::UnboundedReceiver<anyhow::Result<usize>>>>,
    }

    impl TestSource {
        fn new(failures: usize) -> (Self, mpsc::UnboundedSender<anyhow::Result<usize>>) {
            let (sender, receiver) = mpsc::unbounded_channel();
            let source = Self {
                connections: AtomicUsize::new(0),
                failures,
                updates: tokio::sync::Mutex::new(Some(receiver)),
            };

            (source, sender)
        }

        fn connections(&self) -> usize {
            self.connections.load(Ordering::SeqCst)
        }
    }

    impl StateSource for TestSource {
        type State = usize;
        type Error = anyhow::Error;

        async fn connect(&self) -> anyhow::Result<(usize, BoxStream<'_, anyhow::Result<usize>>)> {
            let connection = self.connections.fetch_add(1, Ordering::SeqCst) + 1;
            if connection <= self.failures {
                return Err(anyhow!("failure {connection}"));
            }

            let updates = match self.updates.lock().await.take() {
                Some(mut receiver) => {
                    futures::stream::poll_fn(move |cx| receiver.poll_recv(cx)).boxed()
                }
                None => futures::stream::empty().boxed(),
            };

            Ok((connection, updates))
        }
    }

    fn options() -> Options {
        Options {
            retry_interval: Duration::from_secs(1),
            linger: Duration::from_secs(10),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn nothing_runs_without_listeners() {
        let (source, _updates) = TestSource::new(0);
        let state = LiveState::with_options(source, options());

        tokio::time::sleep(Duration::from_secs(60)).await;
        assert_eq!(state.source().connections(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn listeners_share_state_and_updates() {
        let (source, updates) = TestSource::new(0);
        let state = LiveState::with_options(source, options());

        let mut first = state.subscribe().await;
        assert_eq!(**first.get().value().unwrap(), 1);

        updates.send(Ok(42)).unwrap();
        assert_eq!(**first.changed().await.value().unwrap(), 42);

        let mut second = state.subscribe().await;
        assert_eq!(**second.get().value().unwrap(), 42);
        assert_eq!(state.source().connections(), 1);

        updates.send(Ok(43)).unwrap();
        assert_eq!(**first.changed().await.value().unwrap(), 43);
        assert_eq!(**second.changed().await.value().unwrap(), 43);
    }

    #[tokio::test(start_paused = true)]
    async fn errors_are_served_then_retried() {
        let (source, _updates) = TestSource::new(2);
        let state = LiveState::with_options(source, options());

        let mut subscription = state.subscribe().await;
        let reading = subscription.get();
        assert!(reading.value().is_none());
        assert!(reading.error().is_some());

        let reading = subscription.changed().await;
        assert!(reading.value().is_none());
        assert!(reading.error().is_some());

        assert_eq!(**subscription.changed().await.value().unwrap(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn errors_keep_the_last_good_state() {
        let (source, updates) = TestSource::new(0);
        let state = LiveState::with_options(source, options());

        let mut subscription = state.subscribe().await;
        assert_eq!(**subscription.get().value().unwrap(), 1);

        updates.send(Err(anyhow!("lost connection"))).unwrap();

        let reading = subscription.changed().await;
        assert_eq!(**reading.value().unwrap(), 1);
        assert!(reading.error().is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn update_errors_are_served_then_reconnected() {
        let (source, updates) = TestSource::new(0);
        let state = LiveState::with_options(source, options());

        let mut subscription = state.subscribe().await;
        updates.send(Err(anyhow!("lost connection"))).unwrap();
        assert!(subscription.changed().await.error().is_some());
        assert_eq!(**subscription.changed().await.value().unwrap(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn stops_after_linger() {
        let (source, _updates) = TestSource::new(0);
        let state = LiveState::with_options(source, options());

        drop(state.subscribe().await);

        // Still following, so no new connection is made.
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(**state.get().await.value().unwrap(), 1);

        // `get` itself reset the linger delay.
        tokio::time::sleep(Duration::from_secs(9)).await;
        assert_eq!(**state.get().await.value().unwrap(), 1);

        // Stopped, and the stale state was forgotten.
        tokio::time::sleep(Duration::from_secs(11)).await;
        assert!(state.shared.state.borrow().is_none());
        assert!(!state.shared.control.borrow().running);
        assert_eq!(**state.get().await.value().unwrap(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn cancelled_subscribe_unregisters() {
        let (source, _updates) = TestSource::new(0);
        let state = LiveState::with_options(source, options());

        let subscribe = state.subscribe();
        drop(subscribe);
        assert_eq!(state.shared.control.borrow().listeners, 0);

        // Cancelled while waiting for the first state.
        tokio::select! {
            _ = state.subscribe() => unreachable!(),
            () = std::future::ready(()) => {}
        }
        assert_eq!(state.shared.control.borrow().listeners, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn stream_yields_current_then_updates() {
        let (source, updates) = TestSource::new(0);
        let state = LiveState::with_options(source, options());

        let mut stream = std::pin::pin!(state.subscribe().await.into_stream());
        assert_eq!(**stream.next().await.unwrap().value().unwrap(), 1);

        updates.send(Ok(7)).unwrap();
        assert_eq!(**stream.next().await.unwrap().value().unwrap(), 7);
    }
}

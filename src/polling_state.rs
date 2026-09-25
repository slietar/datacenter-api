use std::future::Future;
use std::time::Duration;

use futures::stream::{BoxStream, StreamExt};
use tracing::Span;

use crate::live_state::{LiveState, Options, Reading, StateSource, Subscription};

/// Something a [`PollingLiveState`] can fetch state from by being asked,
/// rather than by pushing updates of its own.
pub trait Poller: Send + Sync + 'static {
    type State: Send + Sync + std::fmt::Debug + 'static;
    /// The error type yielded when a poll fails, left to the implementor
    /// since it depends on the source.
    type Error: Send + Sync + std::fmt::Debug + 'static;

    /// Fetches the current state.
    fn poll(&self) -> impl Future<Output = Result<Self::State, Self::Error>> + Send;
}

/// Adapts a [`Poller`] into a [`StateSource`] with no updates of its own, so
/// [`LiveState`] re-polls it at [`Options::retry_interval`] instead of
/// listening for pushed ones.
#[derive(Debug)]
struct PollingSource<P>(P);

impl<P: Poller> StateSource for PollingSource<P> {
    type State = P::State;
    type Error = P::Error;

    async fn connect(
        &self,
    ) -> Result<(Self::State, BoxStream<'_, Result<Self::State, Self::Error>>), Self::Error> {
        let state = self.0.poll().await?;
        Ok((state, futures::stream::empty().boxed()))
    }
}

/// A [`LiveState`] for a [`Poller`], re-fetched at [`Options::retry_interval`]
/// rather than following push updates. See [`LiveState`] for when it is
/// connected to.
#[derive(Debug)]
pub struct PollingLiveState<P: Poller>(LiveState<PollingSource<P>>);

impl<P: Poller> Clone for PollingLiveState<P> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<P: Poller> PollingLiveState<P> {
    /// Polls the source every `poll_interval`, using the default
    /// [`Options::linger`].
    pub fn new(poller: P, poll_interval: Duration) -> Self {
        Self::with_options(
            poller,
            Options {
                retry_interval: poll_interval,
                ..Default::default()
            },
        )
    }

    pub fn with_options(poller: P, options: Options) -> Self {
        Self(LiveState::with_options(PollingSource(poller), options))
    }

    /// Sets the span the task polling the source runs in, to tell apart the
    /// logs of different instances.
    pub fn instrument(self, span: Span) -> Self {
        Self(self.0.instrument(span))
    }

    pub fn source(&self) -> &P {
        &self.0.source().0
    }

    /// Registers a new listener, starting to poll the source if nothing was,
    /// and waits until a state is known.
    pub async fn subscribe(&self) -> Subscription<P::State, P::Error> {
        self.0.subscribe().await
    }

    /// Shorthand for subscribing only to get the current state. The source
    /// is still polled for [`Options::linger`], so repeated calls are cheap.
    pub async fn get(&self) -> Reading<P::State, P::Error> {
        self.0.get().await
    }
}

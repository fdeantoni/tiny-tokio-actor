use std::{sync::Mutex, time::Duration};

use backoff::backoff::Backoff as InnerBackoff;

/// A SupervisionStrategy defines what to do when an actor fails at startup.
/// Currently there are two choices: Stop the actor and do nothing, or Retry
/// the startup with one of the built-in strategies or a custom one.
#[derive(Debug)]
pub enum SupervisionStrategy {
    /// Stop the actor immediately on failure
    Stop,
    /// Retry immediately without any delay between attempts
    NoInterval {
        /// Maximum number of retry attempts
        max_retries: usize,
    },
    /// Retry with a fixed delay between attempts
    FixedInterval {
        /// Maximum number of retry attempts
        max_retries: usize,
        /// Duration to wait between retry attempts
        interval: Duration,
    },
    /// Retry with exponential backoff between attempts
    ExponentialBackoff(ExponentialBackoffConfig),
    /// Retry with a custom strategy
    Custom(Box<dyn RetryStrategy>),
}

impl SupervisionStrategy {
    /// Create a strategy that stops the actor on failure (no retries).
    ///
    /// This is the default supervision strategy.
    ///
    /// # Example
    /// ```
    /// use tiny_tokio_actor::SupervisionStrategy;
    ///
    /// let strategy = SupervisionStrategy::stop();
    /// ```
    pub fn stop() -> Self {
        SupervisionStrategy::Stop
    }

    /// Create a strategy that retries immediately without any delay.
    ///
    /// # Example
    /// ```
    /// use tiny_tokio_actor::SupervisionStrategy;
    ///
    /// let strategy = SupervisionStrategy::no_interval(5);
    /// ```
    pub fn no_interval(max_retries: usize) -> Self {
        SupervisionStrategy::NoInterval { max_retries }
    }

    /// Create a strategy that retries with a fixed delay between attempts.
    ///
    /// # Example
    /// ```
    /// use tiny_tokio_actor::{SupervisionStrategy, Duration};
    ///
    /// let strategy = SupervisionStrategy::fixed_interval(5, Duration::from_secs(1));
    /// ```
    pub fn fixed_interval(max_retries: usize, interval: Duration) -> Self {
        SupervisionStrategy::FixedInterval { max_retries, interval }
    }

    /// Create a strategy that retries with exponential backoff between attempts.
    ///
    /// # Example
    /// ```
    /// use tiny_tokio_actor::SupervisionStrategy;
    ///
    /// let strategy = SupervisionStrategy::exponential_backoff(5);
    /// ```
    pub fn exponential_backoff(max_retries: usize) -> Self {
        SupervisionStrategy::ExponentialBackoff(ExponentialBackoffConfig::new(max_retries))
    }

    /// Create a strategy with a custom retry implementation.
    ///
    /// # Example
    /// ```
    /// use tiny_tokio_actor::{SupervisionStrategy, RetryStrategy};
    /// use std::time::Duration;
    ///
    /// #[derive(Debug)]
    /// struct MyStrategy;
    ///
    /// impl RetryStrategy for MyStrategy {
    ///     fn max_retries(&self) -> usize { 3 }
    ///     fn next_backoff(&mut self) -> Option<Duration> { Some(Duration::from_millis(100)) }
    /// }
    ///
    /// let strategy = SupervisionStrategy::custom(MyStrategy);
    /// ```
    pub fn custom<S: RetryStrategy + 'static>(strategy: S) -> Self {
        SupervisionStrategy::Custom(Box::new(strategy))
    }

    /// Returns the maximum number of retries for this strategy, or 0 for Stop.
    pub fn max_retries(&self) -> usize {
        match self {
            SupervisionStrategy::Stop => 0,
            SupervisionStrategy::NoInterval { max_retries } => *max_retries,
            SupervisionStrategy::FixedInterval { max_retries, .. } => *max_retries,
            SupervisionStrategy::ExponentialBackoff(config) => config.max_retries(),
            SupervisionStrategy::Custom(s) => s.max_retries(),
        }
    }

    /// Returns the next backoff duration, if any.
    pub fn next_backoff(&mut self) -> Option<Duration> {
        match self {
            SupervisionStrategy::Stop => None,
            SupervisionStrategy::NoInterval { .. } => None,
            SupervisionStrategy::FixedInterval { interval, .. } => Some(*interval),
            SupervisionStrategy::ExponentialBackoff(config) => config.next_backoff(),
            SupervisionStrategy::Custom(s) => s.next_backoff(),
        }
    }
}

/// Trait to define a RetryStrategy. You can use this trait to define your
/// custom retry strategy.
pub trait RetryStrategy: std::fmt::Debug + Send + Sync {
    /// Maximum number of tries before permanently failing an actor
    fn max_retries(&self) -> usize;
    /// Wait duration before retrying
    fn next_backoff(&mut self) -> Option<Duration>;
}

/// Configuration for exponential backoff retry strategy.
///
/// This strategy retries an actor with an exponentially increasing wait
/// period between retries, using the `backoff` crate internally.
#[derive(Debug, Default)]
pub struct ExponentialBackoffConfig {
    max_retries: usize,
    inner: Mutex<backoff::ExponentialBackoff>,
}

impl ExponentialBackoffConfig {
    /// Create a new exponential backoff configuration with the specified
    /// maximum number of retries.
    pub fn new(max_retries: usize) -> Self {
        ExponentialBackoffConfig {
            max_retries,
            inner: Mutex::new(backoff::ExponentialBackoff::default()),
        }
    }

    /// Returns the maximum number of retries.
    pub fn max_retries(&self) -> usize {
        self.max_retries
    }

    /// Returns the next backoff duration.
    pub fn next_backoff(&mut self) -> Option<Duration> {
        self.inner.lock().ok().and_then(|mut eb| eb.next_backoff())
    }
}

impl RetryStrategy for ExponentialBackoffConfig {
    fn max_retries(&self) -> usize {
        self.max_retries
    }

    fn next_backoff(&mut self) -> Option<Duration> {
        self.inner.lock().ok().and_then(|mut eb| eb.next_backoff())
    }
}

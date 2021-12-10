pub(crate) mod handler;
pub(crate) mod runner;
pub(crate) mod supervision;

use async_trait::async_trait;
use thiserror::Error;

mod path;
pub use path::ActorPath;

use supervision::SupervisionStrategy;

use crate::system::{ActorSystem, SystemEvent};

/// The actor context gives a running actor access to its path, as well as the system that
/// is running it.
pub struct ActorContext<E: SystemEvent> {
    pub path: ActorPath,
    pub system: ActorSystem<E>,
}

impl<E: SystemEvent> ActorContext<E> {
    /// Create a child actor under this actor.
    pub async fn create_child<A: Actor<E>>(
        &self,
        name: &str,
        actor: A,
    ) -> Result<ActorRef<E, A>, ActorError> {
        let path = self.path.clone() / name;
        self.system.create_actor_path(path, actor).await
    }

    /// Retrieve a child actor running under this actor.
    pub async fn get_child<A: Actor<E>>(&self, name: &str) -> Option<ActorRef<E, A>> {
        let path = self.path.clone() / name;
        self.system.get_actor(&path).await
    }

    /// Retrieve or create a new child under this actor if it does not exist yet
    pub async fn get_or_create_child<A, F>(
        &self,
        name: &str,
        actor_fn: F,
    ) -> Result<ActorRef<E, A>, ActorError>
    where
        A: Actor<E>,
        F: FnOnce() -> A,
    {
        let path = self.path.clone() / name;
        self.system.get_or_create_actor_path(&path, actor_fn).await
    }

    /// Stops the child actor
    pub async fn stop_child(&self, name: &str) {
        let path = self.path.clone() / name;
        self.system.stop_actor(&path).await;
    }

    pub(crate) async fn restart<A>(
        &mut self,
        actor: &mut A,
        error: Option<&ActorError>,
    ) -> Result<(), ActorError>
    where
        A: Actor<E>,
    {
        actor.pre_restart(self, error).await
    }
}

/// Defines what an actor will receive as its message, and with what it should respond.
pub trait Message: Clone + Send + Sync + 'static {
    /// response an actor should give when it receives this message. If no response is
    /// required, use `()`.
    type Response: Send + Sync + 'static;
}

/// Defines what the actor does with a message.
#[async_trait]
pub trait Handler<E: SystemEvent, M: Message>: Send + Sync {
    async fn handle(&mut self, msg: M, ctx: &mut ActorContext<E>) -> M::Response;
}

/// Basic trait for actors. Allows you to define tasks that should be run before
/// actor startup, when an actor restarts, and tasks that should be run after
/// the  actor is stopped. It also allows you to define a supervisor strategy
/// that should govern the actor when it fails to start up properly. For
/// example:
/// ```
/// use tiny_tokio_actor::*;
/// use std::time::Duration;
///
/// #[derive(Clone, Debug)]
/// struct TestEvent(String);
///
/// impl SystemEvent for TestEvent {}
///
/// # #[derive(Clone)]
/// # struct Database;
/// # impl Database {
/// #   pub fn init() -> Result<Self, std::io::Error> {
/// #       Ok(Database)
/// #   }
/// # }
///
/// #[derive(Clone)]
/// struct MyActor {
///     db: Option<Database>
/// }
///
/// #[async_trait]
/// impl Actor<TestEvent> for MyActor {
///
///     // If it fails to start up, retry the actor 5 times, with a wait period
///     // of 5 seconds before each retry
///     fn supervision_strategy() -> SupervisionStrategy {
///         let strategy = supervision::FixedIntervalStrategy::new(5, Duration::from_secs(5));
///         SupervisionStrategy::Retry(Box::new(strategy))
///     }
///
///     // Initialize the database
///     async fn pre_start(&mut self, _ctx: &mut ActorContext<TestEvent>) -> Result<(), ActorError> {
///         let db = Database::init().map_err(ActorError::new)?;
///         self.db = Some(db);
///         Ok(())
///     }
///
///     // When restarting, log an error message and try starting again
///     async fn pre_restart(&mut self, ctx: &mut ActorContext<TestEvent>, error: Option<&ActorError>) -> Result<(), ActorError> {
///         log::error!("Actor '{}' is restarting due to {:#?}. Restarting...", ctx.path, error);
///         self.pre_start(ctx).await
///     }
/// }
/// ```
#[async_trait]
pub trait Actor<E: SystemEvent>: Send + Sync + 'static {
    /// Defines the supervision strategy to use for this actor. By default it is
    /// `Stop` which simply stops the actor if an error occurs at startup. You
    /// can also set this to [`SupervisionStrategy::Retry`] with a chosen
    /// [`supervision::RetryStrategy`].
    fn supervision_strategy() -> SupervisionStrategy {
        SupervisionStrategy::Stop
    }

    /// Override this function if you like to perform initialization of the actor
    async fn pre_start(&mut self, _ctx: &mut ActorContext<E>) -> Result<(), ActorError> {
        Ok(())
    }

    /// Override this function if you want to define what should happen when an
    /// error occurs in [`Actor::pre_start()`]. By default it simply calls
    /// `pre_start()` again, but you can also choose to reinitialize the actor
    /// in some other way.
    async fn pre_restart(
        &mut self,
        ctx: &mut ActorContext<E>,
        _error: Option<&ActorError>,
    ) -> Result<(), ActorError> {
        self.pre_start(ctx).await
    }

    /// Override this function if you like to perform work when the actor is stopped
    async fn post_stop(&mut self, _ctx: &mut ActorContext<E>) {}
}

/// A clonable actor reference. It basically holds a Sender that can send messages
/// to the mailbox (receiver) of the actor.
pub struct ActorRef<E: SystemEvent, A: Actor<E>> {
    path: ActorPath,
    sender: handler::HandlerRef<E, A>,
}

impl<E: SystemEvent, A: Actor<E>> Clone for ActorRef<E, A> {
    fn clone(&self) -> Self {
        Self { path: self.path.clone(), sender: self.sender.clone() }
    }
}

impl<E: SystemEvent, A: Actor<E>> ActorRef<E, A> {
    /// Get the path of this actor
    pub fn path(&self) -> &ActorPath {
        &self.path
    }

    /// Get the path of this actor
    #[deprecated(since = "0.2.3", note = "please use `path` instead")]
    pub fn get_path(&self) -> &ActorPath {
        &self.path
    }

    /// Fire and forget sending of messages to this actor.
    pub fn tell<M>(&mut self, msg: M) -> Result<(), ActorError>
    where
        M: Message,
        A: Handler<E, M>,
    {
        self.sender.tell(msg)
    }

    /// Send a message to an actor, expecting a response.
    pub async fn ask<M>(&mut self, msg: M) -> Result<M::Response, ActorError>
    where
        M: Message,
        A: Handler<E, M>,
    {
        self.sender.ask(msg).await
    }

    /// Checks if the actor message box is still open. If it is closed, the actor
    /// is not running.
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    pub(crate) fn new(path: ActorPath, sender: handler::MailboxSender<E, A>) -> Self {
        let handler = handler::HandlerRef::new(sender);
        ActorRef {
            path,
            sender: handler,
        }
    }
}

impl<E: SystemEvent, A: Actor<E>> std::fmt::Debug for ActorRef<E, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

#[derive(Error, Debug)]
pub enum ActorError {
    #[error("Actor exists")]
    Exists(ActorPath),

    #[error("Actor creation failed")]
    CreateError(String),

    #[error("Sending message failed")]
    SendError(String),

    #[error("Actor runtime error")]
    RuntimeError(anyhow::Error),
}

impl ActorError {
    pub fn new<E>(error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::RuntimeError(anyhow::Error::new(error))
    }
}

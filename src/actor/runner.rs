use crate::system::{ActorSystem, SystemEvent};

use super::{
    handler::{ActorMailbox, MailboxReceiver},
    Actor, ActorContext, ActorPath, ActorRef, SupervisionStrategy,
};

pub(crate) struct ActorRunner<E: SystemEvent, A: Actor<E>> {
    path: ActorPath,
    actor: A,
    receiver: MailboxReceiver<E, A>,
}

impl<E: SystemEvent, A: Actor<E>> ActorRunner<E, A> {
    pub fn create(path: ActorPath, actor: A) -> (Self, ActorRef<E, A>) {
        let (sender, receiver) = ActorMailbox::create();
        let actor_ref = ActorRef::new(path.clone(), sender);
        let runner = ActorRunner {
            path,
            actor,
            receiver,
        };
        (runner, actor_ref)
    }

    pub async fn start(&mut self, system: ActorSystem<E>) {
        log::debug!("Starting actor '{}'...", &self.path);

        let mut ctx = ActorContext {
            path: self.path.clone(),
            system: system.clone(),
        };

        // Start the actor
        let mut start_error = self.actor.pre_start(&mut ctx).await.err();

        // Did we encounter an error at startup? If yes, initiate supervision strategy
        if start_error.is_some() {
            let mut retries = 0;
            match A::supervision_strategy() {
                SupervisionStrategy::Stop => {
                    log::error!("Actor '{}' failed to start!", &self.path);
                }
                SupervisionStrategy::Retry(mut retry_strategy) => {
                    log::debug!(
                        "Restarting actor with retry strategy: {:?}",
                        &retry_strategy
                    );
                    while retries < retry_strategy.max_retries() && start_error.is_some() {
                        log::debug!("retries: {}", &retries);
                        if let Some(duration) = retry_strategy.next_backoff() {
                            log::debug!("Backoff for {:?}", &duration);
                            tokio::time::sleep(duration).await;
                        }
                        retries += 1;
                        start_error = ctx
                            .restart(&mut self.actor, start_error.as_ref())
                            .await
                            .err();
                    }
                }
            }
        }

        // No errors encountered at startup, so let's run the actor...
        if start_error.is_none() {
            log::debug!("Actor '{}' has started successfully.", &self.path);

            // If a timeout is set for this actor make sure to apply it
            if let Some(timeout) = A::timeout() {
                log::debug!("Timeout of {:?} set for actor {}", timeout, &self.path);
                while let Ok(Some(mut msg)) = tokio::time::timeout(timeout, self.receiver.recv()).await {
                    msg.handle(&mut self.actor, &mut ctx).await;
                }
                log::debug!("Actor timed out after {:?} of inactivity.", timeout);
            } else {
                // No timeout for this actor so wait indefinitely
                while let Some(mut msg) = self.receiver.recv().await {
                    msg.handle(&mut self.actor, &mut ctx).await;
                }
            }

            // Actor receiver has closed, so stop the actor
            self.actor.post_stop(&mut ctx).await;
            // and remove it from the running system
            system.stop_actor(&self.path).await;

            log::debug!("Actor '{}' stopped.", &self.path);
        }

        self.receiver.close();
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]
    use crate::*;

    use super::*;

    use tokio::time::Duration;

    #[derive(Clone, Debug)]
    struct TestEvent(String);

    impl SystemEvent for TestEvent {}

    #[derive(Clone)]
    struct NoRetryActor;

    #[async_trait]
    impl Actor<TestEvent> for NoRetryActor {
        async fn pre_start(&mut self, ctx: &mut ActorContext<TestEvent>) -> Result<(), ActorError> {
            log::info!("Starting '{}'...", ctx.path);
            let error = std::io::Error::new(std::io::ErrorKind::Interrupted, "Some error");
            Err(ActorError::new(error))
        }
    }

    fn start_system() -> ActorSystem<TestEvent> {
        if std::env::var("RUST_LOG").is_err() {
            unsafe { std::env::set_var("RUST_LOG", "trace") };
        }
        let _ = env_logger::builder().is_test(true).try_init();

        let bus = EventBus::<TestEvent>::new(1000);
        ActorSystem::new("test", bus)
    }

    #[tokio::test]
    async fn no_retry_strategy() {
        let system = start_system();
        let path = ActorPath::from("/test/actor");
        let actor = NoRetryActor;
        let (mut runner, actor_ref) = ActorRunner::create(path, actor);

        runner.start(system).await;

        assert!(actor_ref.is_closed());
    }

    #[derive(Clone, Default)]
    struct RetryNoIntervalActor {
        counter: usize,
    }

    #[async_trait]
    impl Actor<TestEvent> for RetryNoIntervalActor {
        fn supervision_strategy() -> SupervisionStrategy {
            let strategy = supervision::NoIntervalStrategy::new(5);
            SupervisionStrategy::Retry(Box::new(strategy))
        }

        async fn pre_start(&mut self, ctx: &mut ActorContext<TestEvent>) -> Result<(), ActorError> {
            log::info!("Actor '{}' started.", ctx.path);
            self.counter += 1;
            log::info!("Counter is now {}", self.counter);
            let error = std::io::Error::new(std::io::ErrorKind::Interrupted, "Some error");
            Err(ActorError::new(error))
        }

        async fn pre_restart(
            &mut self,
            ctx: &mut ActorContext<TestEvent>,
            error: Option<&ActorError>,
        ) -> Result<(), ActorError> {
            log::info!(
                "Actor '{}' is restarting due to {:#?}. Resetting counter to default",
                ctx.path,
                error
            );
            *self = Self::default();
            self.counter += 1;
            log::info!("Counter is now {}", self.counter);
            let error = std::io::Error::new(std::io::ErrorKind::Interrupted, "Restart error");
            Err(ActorError::new(error))
        }
    }

    #[tokio::test]
    async fn retry_no_interval_strategy() {
        let system = start_system();
        let path = ActorPath::from("/test/actor");
        let actor = RetryNoIntervalActor::default();
        let (mut runner, actor_ref) = ActorRunner::create(path, actor);

        runner.start(system).await;

        assert!(actor_ref.is_closed());
    }

    #[derive(Clone)]
    struct RetryExpBackoffActor {
        counter: usize,
    }

    #[async_trait]
    impl Actor<TestEvent> for RetryExpBackoffActor {
        fn supervision_strategy() -> SupervisionStrategy {
            let strategy = supervision::ExponentialBackoffStrategy::new(5);
            SupervisionStrategy::Retry(Box::new(strategy))
        }

        async fn pre_start(&mut self, ctx: &mut ActorContext<TestEvent>) -> Result<(), ActorError> {
            log::info!("Actor '{}' started.", ctx.path);
            let error = std::io::Error::new(std::io::ErrorKind::Interrupted, "Some error");
            Err(ActorError::new(error))
        }

        async fn pre_restart(
            &mut self,
            ctx: &mut ActorContext<TestEvent>,
            error: Option<&ActorError>,
        ) -> Result<(), ActorError> {
            log::info!("Actor '{}' is restarting due to {:#?}.", ctx.path, error);
            self.counter += 1;
            log::info!("Counter is now {}", self.counter);
            let error = std::io::Error::new(std::io::ErrorKind::Interrupted, "Restart error");
            Err(ActorError::new(error))
        }
    }

    #[tokio::test]
    async fn retry_exponetial_backoff_strategy() {
        let system = start_system();
        let path = ActorPath::from("/test/actor");
        let actor = RetryExpBackoffActor { counter: 0 };
        let (mut runner, actor_ref) = ActorRunner::create(path, actor);

        runner.start(system).await;

        assert!(actor_ref.is_closed());
    }

    struct TimeoutActor;

    impl Actor<TestEvent> for TimeoutActor {
        fn timeout() -> Option<Duration> {
            Some(Duration::from_millis(500))
        }
    }

    #[derive(Clone, Debug)]
    struct TestMessage(usize);

    impl Message for TestMessage {
        type Response = usize;
    }

    #[tokio::test]
    async fn timeout_actor() {
        let system = start_system();
        let path = ActorPath::from("/test/actor");
        let actor = TimeoutActor;
        let (mut runner, actor_ref) = ActorRunner::create(path.clone(), actor);

        let instant = tokio::time::Instant::now();
        runner.start(system.clone()).await;
        let elapsed = instant.elapsed();

        assert!(actor_ref.is_closed());
        assert!(elapsed.as_millis() > 500);
        assert!(system.get_actor::<TimeoutActor>(&path).await.is_none());
    }
}

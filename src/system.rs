use std::{any::Any, collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use crate::{ActorError, ActorPath, actor::{Actor, ActorRef, runner::ActorRunner}, bus::{EventBus, EventConsumer}};

/// Events that this actor system will send
pub trait SystemEvent: Clone + Send + Sync + 'static {}

#[derive(Clone)]
pub struct ActorSystem<E: SystemEvent> {
    name: String,
    actors: Arc<RwLock<HashMap<ActorPath, Box<dyn Any + Send + Sync + 'static>>>>,
    bus: EventBus<E>
}

impl<E: SystemEvent> ActorSystem<E> {

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn publish(&self, event: E) {
        self.bus.send(event).unwrap_or_else(|error| {
            log::error!("Failed to publish event! {}", error.to_string());
            0
        });
    }

    pub fn events(&self) -> EventConsumer<E> {
        self.bus.subscribe()
    }

    pub async fn get_actor<A: Actor<E>>(&self, path: &ActorPath) -> Option<ActorRef<A, E>> {
        let actors = self.actors.read().await;
        actors.get(path).and_then(|any| {
            any.downcast_ref::<ActorRef<A, E>>().cloned()
        })
    }

    pub(crate) async fn create_actor_path<A: Actor<E>>(&self, path: ActorPath, actor: A) -> Result<ActorRef<A, E>, ActorError> {

        log::debug!("Creating actor '{}' on system '{}'...", &path, &self.name);

        let mut actors = self.actors.write().await;
        if actors.contains_key(&path) {
            return Err(ActorError::Create( format!("Actor path '{}' already exists.", &path) ))
        }

        let system = self.clone();
        let (mut runner, actor_ref) = ActorRunner::create(path, actor);
        tokio::spawn( async move {
            runner.start(system).await;
        });

        let path = actor_ref.get_path().clone();
        let any = Box::new(actor_ref.clone());

        actors.insert(path, any);

        Ok(actor_ref)
    }

    pub async fn create_actor<A: Actor<E>>(&self, name: &str, actor: A) -> Result<ActorRef<A, E>, ActorError> {
        let path = ActorPath::from("/user") / name;
        self.create_actor_path(path, actor).await
    }

    pub async fn stop_actor(&self, path: &ActorPath) {
        log::debug!("Stopping actor '{}' on system '{}'...", &path, &self.name);
        let mut paths: Vec<ActorPath> = Vec::new();
        {
            let current_actors = self.actors.read().await;
            for child in current_actors.keys() {
                if child.is_descendant_of(path) {
                    paths.push(child.clone());
                }
            }
        }
        paths.sort_unstable();
        paths.reverse();
        let mut actors = self.actors.write().await;
        for path in &paths {
            actors.remove(path);
        }
    }

    pub fn new(name: &str, bus: EventBus<E>) -> Self {
        let name = name.to_string();
        let actors = Arc::new(RwLock::new(HashMap::new()));
        ActorSystem { name, actors, bus }
    }
}

#[cfg(test)]
mod tests {

    use async_trait::async_trait;
    use crate::actor::{Actor, ActorContext, Handler, Message};

    use super::*;

    #[derive(Clone, Debug)]
    struct TestEvent(String);

    impl SystemEvent for TestEvent {}

    #[derive(Clone)]
    struct TestActor {
        counter: usize
    }

    #[async_trait]
    impl Actor<TestEvent> for TestActor {
        async fn pre_start(&mut self, _ctx: &mut ActorContext<TestEvent>) {
            log::debug!("Starting actor TestActor!");
        }

        async fn post_stop(&mut self, _ctx: &mut ActorContext<TestEvent>) {
            log::debug!("Stopped actor TestActor!");
        }
    }

    #[derive(Clone, Debug)]
    struct TestMessage(usize);

    impl Message for TestMessage {
        type Response = usize;
    }

    impl SystemEvent for TestMessage {}

    #[async_trait]
    impl Handler<TestMessage, TestEvent> for TestActor {
        async fn handle(&mut self, msg: TestMessage, ctx: &mut ActorContext<TestEvent>) -> usize {
            log::debug!("received message! {:?}", &msg);
            self.counter += 1;
            log::debug!("counter is now {}", &self.counter);
            log::debug!("{} on system {}", &ctx.path, ctx.system.get_name());
            ctx.system.publish(TestEvent("Message received!".to_string()));
            self.counter
        }
    }

    #[derive(Clone)]
    struct OtherActor {
        message: String,
        child: Option<ActorRef<TestActor, TestEvent>>
    }

    #[async_trait]
    impl Actor<TestEvent> for OtherActor {
        async fn pre_start(&mut self, ctx: &mut ActorContext<TestEvent>) {
            let child = TestActor { counter: 0 };
            self.child = ctx.create_child("child", child).await.ok();
        }

        async fn post_stop(&mut self, _ctx: &mut ActorContext<TestEvent>) {
            log::debug!("OtherActor stopped.");
        }
    }

    #[derive(Clone, Debug)]
    struct OtherMessage(String);

    impl Message for OtherMessage {
        type Response = String;
    }

    #[async_trait]
    impl Handler<OtherMessage, TestEvent> for OtherActor {
        async fn handle(&mut self, msg: OtherMessage, ctx: &mut ActorContext<TestEvent>) -> String {
            log::debug!("OtherActor received message! {:?}", &msg);
            log::debug!("original message is {}", &self.message);
            self.message = msg.0;
            log::debug!("message is now {}", &self.message);
            log::debug!("{} on system {}", &ctx.path, ctx.system.get_name());
            ctx.system.publish(TestEvent("Received message!".to_string()));
            self.message.clone()
        }
    }

    #[tokio::test]
    async fn actor_create() {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "trace");
        }
        let _ = env_logger::builder().is_test(true).try_init();

        let actor = TestActor { counter: 0 };
        let msg = TestMessage(10);

        let bus = EventBus::<TestEvent>::new(1000);
        let system = ActorSystem::new("test", bus);
        let mut actor_ref = system.create_actor("test-actor", actor).await.unwrap();
        let result = actor_ref.ask(msg).await.unwrap();

        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn actor_stop() {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "trace");
        }
        let _ = env_logger::builder().is_test(true).try_init();

        let actor = TestActor { counter: 0 };
        let msg = TestMessage(10);

        let bus = EventBus::<TestEvent>::new(1000);
        let system = ActorSystem::new("test", bus);

        {
            let mut actor_ref = system.create_actor("test-actor", actor).await.unwrap();
            let result = actor_ref.ask(msg).await.unwrap();

            assert_eq!(result, 1);

            system.stop_actor(actor_ref.get_path()).await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    #[tokio::test]
    async fn actor_events() {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "trace");
        }
        let _ = env_logger::builder().is_test(true).try_init();

        let actor = TestActor { counter: 0 };
        let msg = TestMessage(10);

        let bus = EventBus::<TestEvent>::new(1000);
        let system = ActorSystem::new("test", bus);
        let mut actor_ref = system.create_actor("test-actor", actor).await.unwrap();

        let mut events = system.events();
        tokio::spawn(async move {
            loop {
                match events.recv().await {
                    Ok(event) => println!("Received event! {:?}", event),
                    Err(err) => println!("Error receivng event!!! {:?}", err)
                }
            }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let result = actor_ref.ask(msg).await.unwrap();

        assert_eq!(result, 1);
    }

    #[tokio::test]
    async fn actor_get() {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "trace");
        }
        let _ = env_logger::builder().is_test(true).try_init();

        let actor = TestActor { counter: 0 };

        let bus = EventBus::<TestEvent>::new(1000);
        let system = ActorSystem::new("test", bus);
        let original = system.create_actor("test-actor", actor).await.unwrap();

        if let Some(mut actor_ref) = system.get_actor::<TestActor>(original.get_path()).await {
            let msg = TestMessage(10);
            let result = actor_ref.ask(msg).await.unwrap();
            assert_eq!(result, 1);
        } else {
            panic!("It should have retrieved the actor!")
        }

        if let Some(mut actor_ref) = system.get_actor::<OtherActor>(original.get_path()).await {
            let msg = OtherMessage("Hello world!".to_string());
            let result = actor_ref.ask(msg).await.unwrap();
            println!("Result is: {}", result);
            panic!("It should not go here!");
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn actor_parent_child() {
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "trace");
        }
        let _ = env_logger::builder().is_test(true).try_init();

        let actor = OtherActor {
            message: "Initial".to_string(),
            child: None
        };

        let bus = EventBus::<TestEvent>::new(1000);
        let system = ActorSystem::new("test", bus);

        {
            let mut actor_ref = system.create_actor("test-actor", actor).await.unwrap();
            let msg = OtherMessage("new message!".to_string());
            let result = actor_ref.ask(msg).await.unwrap();
            assert_eq!(result, "new message!".to_string());

            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
            system.stop_actor(actor_ref.get_path()).await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
        let actors = system.actors.read().await;
        for actor in actors.keys() {
            println!("Still active!: {:?}", actor);
        }
        assert_eq!(actors.len(), 0);
    }
}
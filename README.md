# *Tiny Tokio Actor* #

[![crates.io](https://buildstats.info/crate/tiny-tokio-actor)](https://crates.io/crates/tiny-tokio-actor) [![build](https://github.com/fdeantoni/tiny-tokio-actor/actions/workflows/rust.yml/badge.svg)](https://github.com/fdeantoni/tiny-tokio-actor/actions/workflows/rust.yml)

Another actor library! Why another? I really like the actor model for development, and wanted something simple I could use on top of [tokio](https://github.com/tokio-rs/tokio).

```toml
[dependencies]
tiny-tokio-actor = "0.3"
```

Lets define an actor. First import the necessary crate:

```rust
use tiny_tokio_actor::*;
```

Next define the message we will be sending on the actor system's message bus:

```rust
// Define the system event bus message
#[derive(Clone, Debug)]
struct TestEvent(String);

impl SystemEvent for TestEvent {}
```

Next define the actor struct. The actor struct must be Send + Sync but need not
be Clone. When implementing the `Actor` trait, you can opt to override the default
`timeout()`, `supervision_strategy()`, `pre_start()`, `pre_restart()`, and
`post_stop()` methods:

```rust
struct TestActor {
    counter: usize
}

#[async_trait]
impl Actor<TestEvent> for TestActor {

    // This actor will time out after 5 seconds of not receiving a message
    fn timeout() -> Option<Duration> {
        Some(Duration::from_secs(5))
    }

    // This actor will immediately retry 5 times if it fails to start
    fn supervision_strategy() -> SupervisionStrategy {
        let strategy = supervision::NoIntervalStrategy::new(5);
        SupervisionStrategy::Retry(Box::new(strategy))
    }

    async fn pre_start(&mut self, ctx: &mut ActorContext<TestEvent>) -> Result<(), ActorError> {
        ctx.system.publish(TestEvent(format!("Actor '{}' started.", ctx.path)));
        Ok(())
    }

    async fn pre_restart(&mut self, ctx: &mut ActorContext<TestEvent>, error: Option<&ActorError>) -> Result<(), ActorError> {
        log::error!("Actor '{}' is restarting due to {:#?}", ctx.path, error);
        self.pre_start(ctx).await
    }

    async fn post_stop(&mut self, ctx: &mut ActorContext<TestEvent>) {
        ctx.system.publish(TestEvent(format!("Actor '{}' stopped.", ctx.path)));
    }
}
```

Next define a message you want the actor to handle. Note that you also define the
response you expect back from the actor. If you do not want a resposne back you can
simpy use `()` as response type.

```rust
#[derive(Clone, Debug)]
struct TestMessage(String);

impl Message for TestMessage {
    type Response = String;
}
```

Now implement the behaviour we want from the actor when we receive the message:

```rust
#[async_trait]
impl Handler<TestEvent, TestMessage> for TestActor {
    async fn handle(&mut self, msg: TestMessage, ctx: &mut ActorContext<TestEvent>) -> String {
        ctx.system.publish(TestEvent(format!("Message {} received by '{}'", &msg, ctx.path)));
        self.counter += 1;
        "Ping!".to_string()
    }
}
```

You can define more messages and behaviours you want the actor to handle. For example, lets
define an `OtherMessage` we will let our actor handle:

```rust
#[derive(Clone, Debug)]
struct OtherMessage(usize);

impl Message for OtherMessage {
    type Response = usize;
}

// What the actor should do with the other message
#[async_trait]
impl Handler<TestEvent, OtherMessage> for TestActor {
    async fn handle(&mut self, msg: OtherMessage, ctx: &mut ActorContext<TestEvent>) -> usize {
        ctx.system.publish(TestEvent(format!("Message {} received by '{}'", &msg, ctx.path)));
        self.counter += msg.0;
        self.counter
    }
}
```

We can now test out our actor and send the two message types to it:

```rust
#[tokio::test]
async fn multi_message() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "trace");
    }
    let _ = env_logger::builder().is_test(true).try_init();

    let actor = TestActor { counter: 0 };

    let bus = EventBus::<TestEvent>::new(1000);
    let system = ActorSystem::new("test", bus);
    let actor_ref = system.create_actor("test-actor", actor).await.unwrap();

    let mut events = system.events();
    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => println!("Received event! {:?}", event),
                Err(err) => println!("Error receivng event!!! {:?}", err)
            }
        }
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let msg_a = TestMessage("hello world!".to_string());
    let response_a = actor_ref.ask(msg_a).await.unwrap();
    assert_eq!(response_a, "Ping!".to_string());

    let msg_b = OtherMessage(10);
    let response_b = actor_ref.ask(msg_b).await.unwrap();
    assert_eq!(response_b, 11);
}
```

So basically this library provides:

* An actor system with a message bus
* A strongly typed actor with one or more message handlers
* Actors referenced through ActorPaths and ActorRefs
* A supervision stragegy per actor type
* A timeout per actor type

See the [docs](https://docs.rs/tiny-tokio-actor), [examples](https://github.com/fdeantoni/tiny-tokio-actor/tree/main/examples), and [integration tests](https://github.com/fdeantoni/tiny-tokio-actor/tree/main/tests) for more detailed examples.

Library is still incubating! There is still a lot to be done and the API is still unstable! The
todo list so far:

* Supervisor hierarchy
* Create macros to make the defining of actors a lot simpler

Projects / blog posts that are worth checking out:

* [Coerce-rs](https://github.com/LeonHartley/Coerce-rs)
* [Actors with Tokio](https://ryhl.io/blog/actors-with-tokio/)
* [Unbounded channel deadlock risk](https://www.reddit.com/r/rust/comments/ljx7mc/actors_with_tokio)

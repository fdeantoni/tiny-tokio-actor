use tiny_tokio_actor::*;

// The actor struct must derive Clone.
#[derive(Clone)]
struct TestActor {
    counter: usize
}

// Mark the struct as an actor.
impl Actor for TestActor {}

// The message the actor will expect. It must
// derive Clone. Debug is not required.
#[derive(Clone, Debug)]
struct TestMessage(String);

// Mark the message struct as an actor message. Note
// that we also define the response we expect back
// from this message. If no response is desired, just
// use `()`.
impl Message for TestMessage {
    type Response = String;
}

// The event message you may want to publish to the
// system event bus.
#[derive(Clone, Debug)]
struct TestEvent(String);

// Mark the struct as a system event message.
impl SystemEvent for TestEvent {}

// Define the behaviour of the actor. Note that the `handle` method
// has a `String` return type because that is what we defined the
// Response to be of `TestMessage`. As the method is async, we have
// to annotate the implementation with the `async_trait` macro (a
// re-export of the async-trait crate).
#[async_trait]
impl Handler<TestMessage, TestEvent> for TestActor {
    async fn handle(&mut self, msg: TestMessage, ctx: &mut ActorContext<TestEvent>) -> String {
        log::debug!("received message! {:?}", &msg);
        self.counter += 1;
        log::debug!("counter is now {}", &self.counter);
        log::debug!("actor on system {}", ctx.system.get_name());
        ctx.system.publish(TestEvent(format!("message received by '{}'", ctx.id)));
        "Ping!".to_string()
    }
}

#[tokio::test]
async fn simple_message() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "trace");
    }
    let _ = env_logger::builder().is_test(true).try_init();

    // Create the actor
    let actor = TestActor { counter: 0 };
    // Create  the message we will send
    let msg = TestMessage("hello world!".to_string());

    // Create the system event bus
    let bus = EventBus::<TestEvent>::new(1000);
    // Create the actor system with the event bus
    let system = ActorSystem::new("test", bus);
    // Launch the actor on the actor system
    let mut actor_ref = system.create_actor(actor).await;

    // Listen for events on the system event bus
    let mut events = system.events();
    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => println!("Received event! {:?}", event),
                Err(err) => println!("Error receivng event!!! {:?}", err)
            }
        }
    });

    // Wait a little for the actor to start up
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Send the actor the message through an `ask` from which we will get a response
    let response = actor_ref.ask(msg).await.unwrap();

    assert_eq!(response, "Ping!".to_string());
}
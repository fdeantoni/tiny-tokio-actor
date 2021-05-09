use tiny_tokio_actor::*;

// Define the system event bus message
#[derive(Clone, Debug)]
struct TestEvent(String);

impl SystemEvent for TestEvent {}

// Define the actor
#[derive(Clone)]
struct TestActor {
    counter: usize
}

#[async_trait]
impl Actor<TestEvent> for TestActor {
    async fn pre_start(&mut self, ctx: &mut ActorContext<TestEvent>) {
        ctx.system.publish(TestEvent(format!("Actor '{}' started.", ctx.path)));
    }

    async fn post_stop(&mut self, ctx: &mut ActorContext<TestEvent>) {
        ctx.system.publish(TestEvent(format!("Actor '{}' stopped.", ctx.path)));
    }
}

// Define a message the actor will handle
#[derive(Clone, Debug)]
struct TestMessage(String);

impl Message for TestMessage {
    type Response = String;
}

// What the actor should do with TestMessage
#[async_trait]
impl Handler<TestMessage, TestEvent> for TestActor {
    async fn handle(&mut self, msg: TestMessage, ctx: &mut ActorContext<TestEvent>) -> String {
        ctx.system.publish(TestEvent(format!("Message {:?} received by '{}'", &msg, ctx.path)));
        self.counter += 1;
        "Ping!".to_string()
    }
}

// Define another message type the actor will handle
#[derive(Clone, Debug)]
struct OtherMessage(usize);

impl Message for OtherMessage {
    type Response = usize;
}

// What the actor should do with the other message
#[async_trait]
impl Handler<OtherMessage, TestEvent> for TestActor {
    async fn handle(&mut self, msg: OtherMessage, ctx: &mut ActorContext<TestEvent>) -> usize {
        ctx.system.publish(TestEvent(format!("Message {:?} received by '{}'", &msg, ctx.path)));
        self.counter += msg.0;
        self.counter
    }
}

#[tokio::test]
async fn multi_message() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "trace");
    }
    let _ = env_logger::builder().is_test(true).try_init();

    let actor = TestActor { counter: 0 };

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

    let msg_a = TestMessage("hello world!".to_string());
    let response_a = actor_ref.ask(msg_a).await.unwrap();
    assert_eq!(response_a, "Ping!".to_string());

    let msg_b = OtherMessage(10);
    let response_b = actor_ref.ask(msg_b).await.unwrap();
    assert_eq!(response_b, 11);
}
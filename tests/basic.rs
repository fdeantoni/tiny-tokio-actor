use tiny_tokio_actor::{Actor, ActorContext, ActorSystem, EventBus, Handler, Message, SystemEvent};
use async_trait::async_trait;


#[derive(Clone)]
struct TestActor {
    counter: usize
}

#[derive(Clone, Debug)]
struct TestMessage(String);

impl Message for TestMessage {
    type Response = String;
}

#[derive(Clone, Debug)]
struct TestEvent(String);

impl SystemEvent for TestEvent {}

impl Actor for TestActor {}

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

    let actor = TestActor { counter: 0 };
    let msg = TestMessage("hello world!".to_string());

    let bus = EventBus::<TestEvent>::new(1000);
    let system = ActorSystem::new("test", bus);
    let mut actor_ref = system.create_actor(actor).await;

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

    assert_eq!(result, "Ping!".to_string());
}
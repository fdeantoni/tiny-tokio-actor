# *Tiny Tokio Actor* #
[![crates.io](http://meritbadge.herokuapp.com/tiny-tokio-actor)](https://crates.io/crates/tiny-tokio-actor)

Another actor library! Why another? I really like the actor model for development, and wanted something simple I could
use on top of [tokio](https://github.com/tokio-rs/tokio).

```toml
[dependencies]
tiny-tokio-actor = "0.1"
```

Basically it provides:
* An actor system with a message bus
* A strongly typed actor with a single behaviour
* Actors referenced through ActorRefs

See the [docs](https://docs.rs/tiny-tokio-actor) and [integration tests](https://github.com/fdeantoni/tiny-tokio-actor/tree/main/tests) for examples.

There is still a lot to be done and the API will for sure change! The todo list so far:
* Create `ActorPath` similar to that of Akka
* Create macros to make the defining of actors a lot simpler

Projects / blog posts that are worth checking out:
* [Coerce-rs](https://github.com/LeonHartley/Coerce-rs)
* [Actors with Tokio](https://ryhl.io/blog/actors-with-tokio/)
* [Unbounded channel deadlock risk](https://www.reddit.com/r/rust/comments/ljx7mc/actors_with_tokio)

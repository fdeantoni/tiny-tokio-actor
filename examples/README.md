# *Examples* #

The `websockets.rs` example shows how to use actors with Warp websockets. To run it:

    $ cargo run --example websocket

You can then interact with it using websocat:

    $ websocat ws://127.0.0.1:9000/echo


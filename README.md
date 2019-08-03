# An example implementation of an IO event loop

Initially I intended to include this in my book on async code, but the topic of creating a cross platform implementation of an event queue based on OS specific primitives ended up beeing a bigger undertaking than what I thought.

This library aims to simple and focus on explaining the concepts to understand how epoll, kqueue and iocp works, and will for now only support one simple use case, but it's relatively easy to extend once the infrastructure is set up.

Regarding error handling, I will do the basics of error handling but will not cover all the cases, but will consider continue to work on this to create a simple and usable io event queue.

## Usage

```rust
let mut event_loop = EventLoop::new();

let id = event_loop.register_event(Event::new_get("www.google.com"));

while let Ok((event_id, data)) = event_loop.wait() {
    if let id == event_id {
        println!("got a response: ", data);
    }
}
```

## Licence
This library is MIT licensed.

## Contribute
As of this moment, the library is not working or ready yet, but once the main infrastructure is set up and a working example is made I will accept all contributions to make this support the most basic IO operations.


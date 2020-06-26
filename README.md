# An example implementation of an IO event loop

This is the example used in the book [Epoll, Kqueue and IOCP Explained with Rust](https://cfsamsonbooks.gitbook.io/epoll-kqueue-iocp-explained/).

This library aims to be the simplest implementation of a cross platform event loop. It will focus on explaining the concepts to understand how epoll, kqueue and iocp works. For now it will only support one simple use case, that is waiting for a `Read` event on a socket. However, it's relatively easy to extend once the infrastructure is set up to support `Write` events and not only focus on sockets.

The implementation is also designed to be used from one thread registering interests, and another thread waiting for events to occur and handle them. Making a proper multithreaded example is a good exercise, and there are some pointers in the book on how this could be done.

Regarding error handling, I will do the basics of error handling. I have tried to follow best practices, but since this is a self contained example I've not split the code up as much as I probably would otherwise.

## Usage

```rust
struct Reactor {
    handle: Option<JoinHandle<()>>,
    registrator: Option<Registrator>,
}

impl Reactor {
    fn new(evt_sender: Sender<usize>) -> Reactor {
        let mut poll = Poll::new().unwrap();
        let registrator = poll.registrator();

        // Set up the epoll/IOCP event loop in a seperate thread
        let handle = thread::spawn(move || {
            let mut events = Events::with_capacity(1024);
            loop {
                println!("Waiting! {:?}", poll);
                match poll.poll(&mut events, Some(200)) {
                    Ok(..) => (),
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => break,
                    Err(e) => panic!("Poll error: {:?}, {}", e.kind(), e),
                };
                
                for event in &events {
                    let event_token = event.id();
                    evt_sender.send(event_token).expect("Send event_token err.");
                }
            }
        });

        Reactor { handle: Some(handle), registrator: Some(registrator) }
    }

    fn registrator(&mut self) -> Registrator {
        self.registrator.take().unwrap()
    }
}
```

## Expanding on this example
The code is meant to be picked apart and played with. Some good learning projects to do based on the infrastructure could be:
- Rely on the `libc` crate instead of pulling inn constants and definitions by hand. Use C types in the ffi as well
- Make the `Registrator` multithreaded by synchronizing access to it using atomics
- Make a proper `Write` implementation as well

Some more advanced topic could be:
- Handle the edge cases outlined in `It's not that simple` chapter, specifically how to re-register interest if there is more data to read in IOCP than allocated buffers, or if a `poll` starts blocking
- Remove the shortcut we made when we made the socket `blocking` again.
- Consider creating your own sockets instead of using the `TcpStream` from the stdlib so you can have full control over how the socket is instanciated and how DNS-lookup is done

## Licence
This library is MIT licensed.

## Contribute
The main infrastructure is set up and I have a working example used in the book [The Node Experiment - Exploring Async Basics with Rust](https://github.com/cfsamson/book-exploring-async-basics).

It needs to stay current with the book, but I will create a seperate branch for contributions of any kind.


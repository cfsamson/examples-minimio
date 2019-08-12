// use minimio::{Poll, Event, TcpStream, interests::Interests};

// // #[test]
// fn main() {

//     let mut event_loop = EventLoop::new();

//     let id = event_loop.register_event(Event::new_get("www.google.com"));

//     while let Ok((event_id, data)) = event_loop.wait() {
//         if let id == event_id {
//             println!("got a response: ", data);
//         }
//     }
// }

// fn alt2() {
//     let mut poll = Poll::new().unwrap();
//     let registry = poll.registry(); //this is different form mio
//     let mut events = Events::with_capacity(1024);

//     let mut stream = TcpStream::new("www.google.com");

//     // 0 is the ID
//     let future_stream = registry.register(&stream, 0, Interests::Readable).unwrap();

//     loop {
//        poll.poll(&mut events, None).unwrap();
//        for event in events {
//            if event.id() == 0 {
//                // Socket connected (could be a spurious wakeup)
//                let mut buffer = String::new();
//                stream.read_to_string(&mut buffer).unwrap();
//                println!(buffer);
//            }
//        }
//     }

// }



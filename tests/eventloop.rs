use minimio::{Poll, Events, TcpStream, Interests, Registry};
use std::thread;
use std::sync::mpsc::channel;

// fn main() {
//     // First lets set up a "runtime"
//     let poll = Poll::new();
//     let registrar = poll.registry();
//     let (evt_sender, evt_reciever) = channel();

//     let rt = Runtime {
//         events: vec![],
//         registrar,
//     };

//     // Set up the event loop
//     thread::spawn(move || {
//         loop {
//             if let Ok(event) = poll.poll() {
//                 println!("EVENT OCCURED: {}", event);
//                 evt_sender.send(event);
//             }
//         }
//     });

//     // ===== THIS IS "APPLICATION" CODE USING OUR INFRASTRUCTURE =====
//     rt.spawn(|| {
//         let mut stream = TcpStream::connect("slowwly.robertomurray.co.uk:80").unwrap();
//         let request = "GET /delay/1000/url/http://www.google.com HTTP/1.1\r\n\
//                            Host: slowwly.robertomurray.co.uk\r\n\
//                            Connection: close\r\n\
//                            \r\n";
//         stream.write_all(request.as_bytes()).expect("Error writing to stream");

//         // This is where we'll have a problem since read is resolved in our eventloop

//         // we could solve it like this - i think this is what we want, nice preperation for next book
//         let fut = stream.read(); // we just subscribe for a read event...
//         fut.and_then(|stream| {
//             // when the event occurs this is our callback
//             // read the data etc
//         });

//         // at some point waker.wake() is called, we could mimic this by just changing a
//         // AtomicBool on that task marking it as ready.

//         loop {
//             match fut.poll() {
//                 Ok(res) => {
//                     match res {
//                         Async::Ready(data) => (), // poll once more and remove the event from the poll list
//                         Async::NotReady => (),
//                     }
//                 },
//                 Err(e) => panic!(e),
//             }
//         }

//         // or we could do it like this
//         stream.read(|reader| {
//             // read from the stream here
//         }).exepct("Error reading from stream.");

//         // Mio does this
//         poll.registry().register(&stream, 10, Interests::readable());
//         let mut events = Events::with_capcacity(1024);

//         loop {
//             poll.poll(&mut events);
//             for event in events {
//                 if event.token() == 10 {
//                     // our socket is ready
//                     stream.read();
//                 }
//             }
//         }

//         let callback_to_call = move || {
//             let mut buffer = String::new();
//             stream.read_to_string(&mut buffer).unwrap(); // will fail if we call it too soon
//             println!("{}", buffer);
//         }
//     });

//     // lets just wait for events in this test 
//     while let Ok(event) = evt_reciever.recv() {
//         // Running the code for event
//         rt.run(event);
//     }
// }


// struct Runtime {
//     events: Vec<(usize, Box<dyn Fn()>)>,
//     registrar: Registry,
// }

// impl Runtime {
//     fn spawn(&mut self, f: impl Fn() + 'static) {
//         let id = self.events.len();
//         self.events.push((id, Box::new(f)));
//     }

//     fn run(&mut self, event: usize) {
//         let (_, f) = self.events.iter().find(|(event, _)| event == event).expect("Couldn't find event.");

//         f();
//     }
// }
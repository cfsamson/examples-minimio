use minimio::{Poll, Events, TcpStream, Interests, Token};
use std::thread;
use std::sync::mpsc::channel;
use std::io::{Read, Write};

#[test]
fn main() {
    // First lets set up a "runtime"
    let poll = Poll::new().unwrap();
    let (evt_sender, evt_reciever) = channel();

    let mut rt = Runtime {
        events: vec![]
    };

    // Set up the epoll/IOCP event loop
    thread::spawn(move || {
        let mut events = Events::with_capacity(1024);
        loop {
            poll.poll(&mut events);
            for event in &events {
                evt_sender.send(event.id());
            }
        }
    });

    // ===== THIS IS "APPLICATION" CODE USING OUR INFRASTRUCTURE =====
    let mut stream = TcpStream::connect("slowwly.robertomurray.co.uk:80").unwrap();
    let request = "GET /delay/1000/url/http://www.google.com HTTP/1.1\r\n\
                           Host: slowwly.robertomurray.co.uk\r\n\
                           Connection: close\r\n\
                           \r\n";
    stream.write_all(request.as_bytes()).expect("Error writing to stream");

    // Mio does this
    // NOTE: On windows, the TcpStream struct can contain an Arc<Mutex<Vec<u8>>> where it leaves
    // a reference to the buffer with our selector that which can fill it when data is ready
    poll.registry().register_with_id(&stream, Interests::readable(), 10);

    // When we get notified that 10 is ready we can run this code
    rt.spawn(10, move || {
        let mut buffer = String::new();
        stream.read_to_string(&mut buffer).unwrap();
        println!("{}", buffer);
    });



    // ===== THIS WILL BE IN OUR MAIN EVENT LOOP ======
    // But we'll only check if we have gotten anything, not block 
    while let Ok(event) = evt_reciever.recv() {
        // Running the code for event
        rt.run(event.value()); // runs the code associated with event 10 in this case
    }
}


struct Runtime {
    events: Vec<(usize, Box<dyn Fn()>)>,
}

impl Runtime {
    fn spawn(&mut self, id: usize, f: impl Fn() + 'static) {
        self.events.push((id, Box::new(f)));
    }

    fn run(&mut self, event: usize) {
        let (_, f) = self.events.iter().find(|(event, _)| event == event).expect("Couldn't find event.");
        f();
    }
}
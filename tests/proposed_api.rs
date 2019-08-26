use minimio::{Poll, Events, TcpStream, Interests, Token};
use std::thread;
use std::sync::mpsc::channel;
use std::io::{Read, Write};

#[test]

fn proposed_api() {
    // First lets set up a "runtime"
    let mut poll = Poll::new().unwrap();
    let registrator = poll.registrator();

    let (evt_sender, evt_reciever) = channel();

    let mut rt = Runtime {
        events: vec![]
    };

    // Set up the epoll/IOCP event loop
    let handle = thread::spawn(move || {
        let mut events = Events::with_capacity(1024);
        loop {
            println!("POLLING");
            let mut will_close = false;
            poll.poll(&mut events).unwrap();
            for event in &events {
                println!("GOT EVENT: {:?}", event.token().unwrap().value());
                let value = event.token().unwrap().value();
                if value == u32::max_value() as usize {
                    will_close = true;
                } else {
                    evt_sender.send(value).unwrap();
                }
            }

            if will_close {
                break;
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
    
    // PROBLEM 2: We need to use registry here
    registrator.register(&mut stream, 10, Interests::readable()).unwrap();
    println!("HERE");

    // When we get notified that 10 is ready we can run this code
    rt.spawn(10, move || {
        let mut buffer = String::new();
        stream.read_to_string(&mut buffer).unwrap();
        println!("PROPOSED API:\n{}", buffer);
    });



    // ===== THIS WILL BE IN OUR MAIN EVENT LOOP ======
    // But we'll only check if we have gotten anything, not block 
    println!("WAITING FOR EVENTS");
    while let Ok(token) = evt_reciever.recv() {
        println!("RECIEVED EVENT: {:?}", token);
        // Running the code for event
        rt.run(token); // runs the code associated with event 10 in this case
        // let's close the event loop since we know we only have 1 event
        registrator.close_loop().unwrap();
    }

    println!("EXITING");
}


struct Runtime {
    events: Vec<(usize, Box<dyn FnMut()>)>,
}

impl Runtime {
    fn spawn(&mut self, id: usize, f: impl FnMut() + 'static) {
        self.events.push((id, Box::new(f)));
    }

    fn run(&mut self, event: usize) {
        println!("RUNNING EVENT: {}", event);
        let (_, f) = self.events.iter_mut().find(|(event, _)| event == event).expect("Couldn't find event.");
        f();
    }
}


// The plan:
// 
// Poll {
//     selector: Arc<Selector>,
// }
// 
// fn selector() -> Arc<Selector> // this can be used even though poll is moved
// 
// which means we can do:
// selector.register(...)
// 
// Now register() can't mutate our TcpStream, it can however get the Socket Handle from it
// 
// When we register a "socket"
// -> we create a buffer in Selecor together with the Token
//    (this buffer gets filled on completion)
// 
// When poll returns we know that event X has occurred and the buffer that is stored in Selector is filled
//  -> But how do we get this data to our TcpStream??
// 
// We have event X with the Token and the data 
// 
// We know a callback with Socket X is waiting to be ran when Event X happens
// 
// But we have no communication between Socket X and Selector...
// 
// Now...
// 
// poll.register()
// could instead return both a Poll instance and a ReadinessQueue
// 
// Register works as suggested above but select retains a Arc<Mutex<Vec<u8>>> to the streams buffer.
// ONCE the event is completed and the primary buffer is filled, we fill the Arc<Mutex<>> from TcpStream on windows
//  -> On Unix we do nothing
// 
// the poll instance could be moved
// and we retain the channel and we could call readiness.get_events() -> and retrieve any events 
// and we can use regisrator.register() as before
use minimio::{Poll, Events, TcpStream, Interests, Registry};
use std::thread;
use std::sync::mpsc::channel;

#[test]
fn main() {
    // First lets set up a "runtime"
    let poll = Poll::new();
    let registrar = poll.registry();
    let (evt_sender, evt_reciever) = channel();

    let rt = Runtime {
        events: vec![],
        registrar,
    };

    // Set up the event loop
    thread::spawn(move || {
        loop {
            if let Ok(event) = poll.poll() {
                println!("EVENT OCCURED: {}", event);
                evt_sender.send(event);
            }
        }
    });

    // ===== THIS IS "APPLICATION" CODE USING OUR INFRASTRUCTURE =====
    rt.spawn(|| {
        let mut stream = TcpStream::new();
    })

    // lets just wait for events in this test 
    while let Ok(event) = evt_reciever.recv() {
        // Running the code for event
        rt.run(event);
    }
}


struct Runtime {
    events: Vec<(usize, Box<dyn Fn()>)>,
    registrar: Registry,
}

impl Runtime {
    fn spawn(&mut self, f: impl Fn() + 'static) {
        let id = self.events.len();
        self.events.push((id, Box::new(f)));
    }

    fn run(&mut self, event: usize) {
        let (_, f) = self.events.iter().find(|(event, _)| event == event).expect("Couldn't find event.");

        f();
    }
}
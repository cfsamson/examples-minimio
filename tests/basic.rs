use minimio::{EventLoop, Event, PollStatus};

// #[test]
fn main() {

    let mut event_loop = EventLoop::new();

    let id = event_loop.register_event(Event::new_get("www.google.com"));

    while let Ok((event_id, data)) = event_loop.wait() {
        if let id == event_id {
            println!("got a response: ", data);
        }
    }
}

fn alt2() {
    let mut evtl = EventLoop::new();

    let mut stream = minimio::TcpStream::new("www.google.com");

    let future_stream = evtl.register(stream);

    loop {
        match future_stream.poll() {
            PollStatus::WouldBlock => (),
            PollStatus::Ready(reader) => {
                let mut buff = String::new();
                reader.read_to_string(&mut buff);
                println!("{}", buff);
            }, 
            PollStatus::Finished => break,
        }
    }

}



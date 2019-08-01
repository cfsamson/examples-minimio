use minimio::{EventLoop, Event};

#[test]
fn main() {

    let mut event_loop = EventLoop::new();

    let id = event_loop.register_event(Event::new_get("www.google.com"));

    while let Ok(event_id, data) = event_loop.wait() {
        if let id == event_id {
            println!("got a response: ", data);
        }
    }
}



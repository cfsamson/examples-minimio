use minimio::{EventLoop, Event};

#[test]
fn main() {

    let mut event_loop = EventLoop::new();

    event_loop.register_event(Event::new_get("www.google.com"));

    loop {
        match event_loop.poll() {
            Some(events) => {
                for event in events {
                    // get callback for event_id
                    let cb = |s: String| { println!("CB for event: {}", s)};
                    cb(event.data());
                }
            },
            None => (),
        }
    }

}



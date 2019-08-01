

#[cfg(target_os="windows")]
pub use windows::{Event, EventLoop, EventResult};
//#[cfg(target_os="linux")]
//pub use linux::{Event, EventLoop, EventResult};

const MAXEVENTS: usize = 1000;


mod windows {
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use super::MAXEVENTS;

    pub struct Event<T> {
        data: T,
    }
    
    
    impl Event<String> {
        pub fn new_get(url: &str) -> Self {
            Event {
                data: String::new(),
            }
        }

        pub fn data(self) -> String {
            self.data
        }
    }

    enum ErrorTypes {
        NoError = 0,
        ChannelError = 1,
    }

    impl From<usize> for ErrorTypes {
        fn from(n: usize) -> Self {
            use ErrorTypes::*;
            match n {
                0 => NoError,
                1 => ChannelError,
                _ => panic!("Invalid error code"),
            }
        }
    }

    pub struct EventLoop {
        has_error: Arc<AtomicUsize>,
        errors: Arc<Mutex<Vec<String>>>,
    }

    impl EventLoop {
        pub fn new() -> Self {
            let has_error = Arc::new(AtomicUsize::new(0));
            let errors = Arc::new(Mutex::new(vec![]));

            let (loop_event_tx, loop_event_rx) = channel::<ffi::IOCPevent>();

            let errors_cloned = errors.clone();
            let has_error_clone = has_error.clone();
            thread::spawn(move || {
                let events = vec![ffi::IOCPevent::default(); MAXEVENTS];
                loop {
                    //wait for events
                    // handle recieved events
                    let n = 0;
                    let iocp_event = events[n].clone();
                    if let Err(e) = loop_event_tx.send(iocp_event) {
                        has_error_clone.store(1, Ordering::Relaxed);
                        let mut guard = errors_cloned.lock().expect("Mutex Poisoned");
                        (&mut guard).push(format!("Error: {:?}\nEvent: {:?}", e, events[n]));
                    }
                }
            });

            EventLoop {
                has_error,
                errors,
            }
        }

        pub fn register_event<T>(&mut self, event: Event<T>) {

        }

        pub fn poll<T>(&mut self) -> Option<Vec<Event<T>>> {
            None
        }

        fn check_errors(&self) -> Option<Vec<String>> {
            if self.has_error.load(Ordering::Relaxed) > 0 {
                let lock = self.errors.lock().expect("Mutex poisioned!");
                let errors = (&lock).iter().map(|s| s.clone()).collect();
                return Some(errors);
            }

            None
        }


    }

    pub enum EventResult {

    }

    mod ffi {
        #[derive(Debug, Clone)]
        pub struct IOCPevent {

        }

        impl Default for IOCPevent {
            fn default() -> Self {
                IOCPevent {

                }
            }
        }
    }
}




#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(true);
    }
}
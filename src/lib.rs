use std::error;
use std::fmt;
use std::io::{self, Read};
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{Event, Selector, TcpStream};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{Event, Selector, TcpStream};
//#[cfg(target_os="linux")]
//pub use linux::{Event, EventLoop, EventResult};

#[cfg(target_os = "macos")]
pub type Source = std::os::unix::io::RawFd;

#[cfg(target_os = "windows")]
pub type Source = std::os::windows::io::RawSocket;

pub type Events = Vec<Event>;

const MAXEVENTS: usize = 1000;
static ID: Id = Id(AtomicUsize::new(0));

struct Id(AtomicUsize);
impl Id {
    fn next(&self) -> usize {
        self.0.fetch_add(1, Ordering::Relaxed)
    }
    
    fn value(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

pub struct Poll {
    registry: Registry,
}

pub struct Registry {
    selector: Selector,
}

impl Poll {
    pub fn new() -> io::Result<Poll> {
        Selector::new().map(|selector| {
            Poll {
                registry: Registry { selector }
            }
        })
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn poll(&mut self, events: &mut Events) -> io::Result<usize> {
        loop {
            let res = self.registry.selector.select(events);

            match res {
                Ok(()) => break,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
            };
        }

        Ok(events.len())
    }
}

impl Registry {
    pub fn register(&self, source: Source, token: Id, interests: Interests) -> io::Result<Id> {
        let t = ID.next();
        self.selector.register(source, token, interests)?;
        Ok(token)
    }
}

    pub const WRITABLE: u8 = 0b0000_0001;
    pub const READABLE: u8 = 0b0000_0010;

    pub struct Interests(u8);
    impl Interests {
        pub fn readable() -> Self {
            Interests(READABLE)
        }
    }
    impl Interests {
        pub fn is_readable(&self) -> bool {
            self.0 & READABLE != 0
        }

        pub fn is_writable(&self) -> bool {
            self.0 & WRITABLE != 0
        }
    }

pub enum PollStatus<T: Read> {
    WouldBlock,
    Ready(T),
    Finished,
}



#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(true);
    }
}

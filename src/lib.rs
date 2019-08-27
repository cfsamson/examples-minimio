use std::error;
use std::fmt;
use std::io::{self, Read};
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{Event, Selector, TcpStream, Registrator};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{Event, Selector, TcpStream, Source, Registrator};
//#[cfg(target_os="linux")]
//pub use linux::{Event, EventLoop, EventResult};


pub type Events = Vec<Event>;

const MAXEVENTS: usize = 1000;
static TOKEN: Token = Token(AtomicUsize::new(0));

pub struct Token(AtomicUsize);
impl Token {
    pub fn next(&self) -> usize {
        self.0.fetch_add(1, Ordering::Relaxed)
    }

    pub fn value(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }

    pub fn new(val: usize) -> Self {
        Token(AtomicUsize::new(val))
    }
}

impl std::cmp::PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        self.value() == other.value()
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

    pub fn registrator(&self) -> Registrator {
        self.registry.selector.registrator()
    }

    pub fn register_with_id(&self, stream: &mut TcpStream, interests: Interests, token: usize) -> io::Result<Token> {
        self.registry.selector.registrator().register(stream, token, interests)?;
        Ok(Token::new(token))
    }

    pub fn register(&self, stream: &mut TcpStream, interests: Interests) -> io::Result<Token> {
        let token = TOKEN.next();
        self.register_with_id(stream, interests, token)
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

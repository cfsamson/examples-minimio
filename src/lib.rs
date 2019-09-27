use std::io;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::fmt;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{Event, Registrator, Selector, TcpStream};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{Event, Registrator, Selector, Source, TcpStream};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{Event, Registrator, Selector, TcpStream};

pub type Events = Vec<Event>;
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

#[derive(Debug)]
pub struct Poll {
    registry: Registry,
    is_poll_dead: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct Registry {
    selector: Selector,
}

impl Poll {
    pub fn new() -> io::Result<Poll> {
        Selector::new().map(|selector| Poll {
            registry: Registry { selector },
            is_poll_dead: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn registrator(&self) -> Registrator {
        self.registry
            .selector
            .registrator(self.is_poll_dead.clone())
    }

    pub fn register_with_id(
        &self,
        stream: &mut TcpStream,
        interests: Interests,
        token: usize,
    ) -> io::Result<Token> {
        self.registry
            .selector
            .registrator(self.is_poll_dead.clone())
            .register(stream, token, interests)?;
        Ok(Token::new(token))
    }

    pub fn register(&self, stream: &mut TcpStream, interests: Interests) -> io::Result<Token> {
        let token = TOKEN.next();
        self.register_with_id(stream, interests, token)
    }

    /// Polls the event loop. The thread yields to the OS while witing for either
    /// an event to retur or a timeout to occur. A negative timeout will be treated
    /// as a timeout of 0.
    pub fn poll(&mut self, events: &mut Events, timeout_ms: Option<i32>) -> io::Result<usize> {
        // A negative timout is converted to a 0 timeout
        let timeout = timeout_ms.map(|n| {
            if n < 0 {
                0
            } else {
                n
            }
        });
        loop {
            let res = self.registry.selector.select(events, timeout);
            match res {
                Ok(()) => break,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
            };
        }

        if self.is_poll_dead.load(Ordering::SeqCst) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Poll closed."));
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


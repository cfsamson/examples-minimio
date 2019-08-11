use std::error;
use std::fmt;
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{Selector, Event, TcpStream};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{Selector, Event, TcpStream};
//#[cfg(target_os="linux")]
//pub use linux::{Event, EventLoop, EventResult};

const MAXEVENTS: usize = 1000;
static ID: Id = Id(AtomicUsize::new(0));

struct Id(AtomicUsize);
impl Id {
    fn next(&self) -> usize {
        self.0.fetch_add(1, Ordering::Relaxed)
    }
}

pub mod Interests {
    pub const WRITABLE: u8 = 0b0000_0001;
    pub const READABLE: u8 = 0b0000_0010;

    pub struct Interests(u8);
    impl Interests {
        pub fn is_readable(&self) -> bool {
            self.0 & READABLE != 0
        }

        pub fn is_writable(&self) -> bool {
            self.0 & WRITABLE != 0
        }
    }
}

pub enum PollStatus<T: Read> {
    WouldBlock,
    Ready(T),
    Finished,
}

#[derive(Debug)]
pub enum ElErr {
    IoError(String),
}

impl fmt::Display for ElErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ElErr::*;
        match self {
            IoError(s) => write!(f, "IO Error: {}", s),
        }
    }
}

impl error::Error for ElErr {}

impl From<std::io::Error> for ElErr {
    fn from(e: std::io::Error) -> ElErr {
        ElErr::IoError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(true);
    }
}

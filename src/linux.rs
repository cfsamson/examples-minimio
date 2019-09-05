use crate::{Events, Interests, Token, STOP_SIGNAL, TOKEN};
use std::io::{self, IoSliceMut, Read, Write};
use std::net;
use std::os::unix::io::{AsRawFd, RawFd};
use std::ptr;

pub struct Registrator {
    kq: Source,
}

impl Registrator {
    pub fn register(
        &self,
        stream: &TcpStream,
        token: usize,
        interests: Interests,
    ) -> io::Result<()> {
        let fd = stream.as_raw_fd();
        if interests.is_readable() {
            // We register the id (or most oftenly referred to as a Token) to the `udata` field
            // if the `Kevent`
            let kevent = ffi::Event::new_read_event(fd, token as u64);
            let kevent = [kevent];
            ffi::syscall_kevent(self.kq, &kevent, &mut [], 0)?;
        };

        if interests.is_writable() {
            unimplemented!();
        }

        Ok(())
    }

    pub fn close_loop(&self) -> io::Result<()> {
        let kevent = ffi::Event::new_wakeup_event();
        let kevent = [kevent];
        ffi::syscall_kevent(self.kq, &kevent, &mut [], 0)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct Selector {
    id: usize,
    kq: Source,
}

impl Selector {
    fn new_with_id(id: usize) -> io::Result<Self> {
        Ok(Selector {
            id,
            kq: ffi::queue()?,
        })
    }

    pub fn new() -> io::Result<Self> {
        Selector::new_with_id(TOKEN.next())
    }

    pub fn id(&self) -> usize {
        self.id
    }

    /// This function blocks and waits until an event has been recieved. It never times out.
    pub fn select(&self, events: &mut Events) -> io::Result<()> {
        // TODO: get n_events from self
        let n_events = events.capacity() as i32;
        events.clear();
        ffi::syscall_kevent(self.kq, &[], events, n_events).map(|n_events| {
            // This is safe because `syscall_kevent` ensures that `n_events` are
            // assigned. We could check for a valid token for each event to verify so this is
            // just a performance optimization used in `mio` and copied here.
            unsafe { events.set_len(n_events as usize) };
        })
    }

    pub fn registrator(&self) -> Registrator {
        Registrator { kq: self.kq }
    }
}

pub type Event = ffi::Kevent;
impl Event {
    pub fn id(&self) -> Token {
        Token::new(self.udata as usize)
    }
}

pub struct TcpStream {
    inner: net::TcpStream,
}

impl TcpStream {
    pub fn connect(adr: impl net::ToSocketAddrs) -> io::Result<Self> {
        // actually we should set this to non-blocking before we call connect which is not something
        // we get from the stdlib but could do with a syscall. Let's skip that step in this example.
        // In other words this will block shortly establishing a connection to the remote server
        let stream = net::TcpStream::connect(adr)?;
        stream.set_nonblocking(true)?;

        Ok(TcpStream { inner: stream })
    }

    pub fn source(&self) -> Source {
        self.inner.as_raw_fd()
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // If we let the socket operate non-blocking we could get an error of kind `WouldBlock`,
        // that means there is more data to read but we would block if we waited for it to arrive.
        // The right thing to do is to re-register the event, getting notified once more
        // data is available. We'll not do that in our implementation since we're making an example
        // and instead we make the socket blocking again while we read from it
        self.inner.set_nonblocking(false)?;
        (&self.inner).read(buf)
    }

    /// Copies data to fill each buffer in order, with the final buffer possibly only beeing
    /// partially filled. Now as we'll see this is like it's made for our use case when abstracting
    /// over IOCP AND epoll/kqueue (since we need to buffer anyways).
    ///
    /// IoSliceMut is like `&mut [u8]` but it's guaranteed to be ABI compatible with the `iovec`
    /// type on unix platforms and `WSABUF` on Windows. Perfect for us.
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut]) -> io::Result<usize> {
        (&self.inner).read_vectored(bufs)
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl AsRawFd for TcpStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

mod ffi {
    use std::io;

    pub struct Event {}

    #[link(name = "c")]
    extern "C" {
        /// http://man7.org/linux/man-pages/man2/epoll_create1.2.html
        pub fn epoll_create(size: i32) -> i32;

        /// http://man7.org/linux/man-pages/man2/close.2.html
        pub fn close(fd: i32) -> i32;

        /// http://man7.org/linux/man-pages/man2/epoll_ctl.2.html
        pub fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: *mut Event) -> i32;

        /// http://man7.org/linux/man-pages/man2/epoll_wait.2.html
        ///
        /// - epoll_event is a pointer to an array of Events
        pub fn epoll_wait(epfd: i32, events: *mut Event, maxevents: i32, timeout: i32) -> i32;
    }

}

fn epoll_create() -> io::Result<i32> {
    let res = unsafe { ffi::epoll_create(0) };
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(res)
    }
}

fn close_fd(fd: i32) -> io::Result<()> {
    let res = unsafe { ffi::close(fd) };
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: &mut Event) -> io::Result<()> {
    let res = unsafe { ffi::epoll_ctl(epfd, op, fd, event) };
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Waits for events on the epoll instance to occur. Returns the number file descriptors ready for the requested I/O.
fn epoll_wait(epfd: i32, events: &mut [Event], maxevents: i32, timeout: i32) -> io::Result<i32> {
    let res = unsafe { ffi::epoll_wait(epfd, events, maxevents, timeout) };
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(res)
    }
}

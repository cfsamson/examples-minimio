use std::net;
use std::io::{self, Read, IoSliceMut};
use std::os::unix::io::{AsRawFd, RawFd};
use crate::ID;

pub struct Selector {
    id: usize,
    kq: RawFd,
}

impl Selector {
    fn new_with_id(id: usize) -> io::Result<Self> {
        Ok(Selector {
            id,
            kq: ffi::queue()?,
        })
    }

    fn new() -> io::Result<Self> {
        Selector::new_with_id(ID.next())
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn select()
}

pub type Event = ffi::Kevent;

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

        Ok(TcpStream {
            inner: stream,
        })
    }
}

impl<'a> Read for &'a TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // if we get the error kind WouldBlock, that means there is more data to read
        // and the right thing to do is to re-register the event, getting notified once more
        // data is available. We'll not do that in our implementation since we're making an example
        match (&self.inner).read(buf) {
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    // instead we do this shortcut: if there is more data to read we just block
                    // and wait for it
                    self.inner.set_nonblocking(false);
                    return self.inner.read_to_end(&mut buf);
                }
            }
        }
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

mod ffi {
    use super::*;

    pub const EVFILT_READ: i16 = -1;
    pub const EV_ADD: u16 = 0x1;
    pub const EV_ENABLE: u16 = 0x4;
    pub const EV_ONESHOT: u16 = 0x10;

    pub type Event = Kevent;
    impl Event {
        fn read_event(fd: RawFd) -> Self {
            Event {
            ident: fd as u64,
            filter: EVFILT_READ,
            flags: EV_ADD | EV_ENABLE | EV_ONESHOT,
            fflags: 0,
            data: 0,
            udata: 0,
        }
        }
    }

    pub fn queue() -> io::Result<i32> {
        let fd = unsafe { ffi::kqueue() };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(fd)
    }

    pub fn create_kevent (
        kq: RawFd,
        cl: &[Kevent],
        el: &mut [Kevent],
        timeout: usize,
    ) -> io::Result<usize> {
        let res = unsafe {
            let kq = kq as i32;
            let cl_len = cl.len() as i32;
            let el_len = el.len() as i32;
            kevent(kq, cl.as_ptr(), cl_len, el.as_mut_ptr(), el_len, timeout)
        };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(res as usize)
    }

        // https://github.com/rust-lang/libc/blob/c8aa8ec72d631bc35099bcf5d634cf0a0b841be0/src/unix/bsd/apple/mod.rs#L497
        // https://github.com/rust-lang/libc/blob/c8aa8ec72d631bc35099bcf5d634cf0a0b841be0/src/unix/bsd/apple/mod.rs#L207
        #[derive(Debug, Clone, Default)]
        #[repr(C)]
        pub struct Kevent {
            pub ident: u64,
            pub filter: i16,
            pub flags: u16,
            pub fflags: u32,
            pub data: i64,
            pub udata: u64,
        }

        #[link(name = "c")]
        extern "C" {
            /// Returns: positive: file descriptor, negative: error
            pub(super) fn kqueue() -> i32;
            /// Returns: nothing, all non zero return values is an error
            pub(super) fn kevent(
                kq: i32,
                changelist: *const Kevent,
                nchanges: i32,
                eventlist: *mut Kevent,
                nevents: i32,
                timeout: usize,
            ) -> i32;
        }
}
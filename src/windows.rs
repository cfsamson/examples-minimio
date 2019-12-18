#![allow(non_camel_case_types)]
#![allow(dead_code)]

use crate::{Interests, Token};
use std::collections::LinkedList;
use std::io::{self, Read, Write};
use std::net;
use std::os::windows::io::{AsRawSocket, RawSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub type Event = ffi::OVERLAPPED_ENTRY;

#[derive(Debug)]
pub struct TcpStream {
    inner: net::TcpStream,
    buffer: Vec<u8>,
    wsabuf: Vec<ffi::WSABUF>,
    event: Option<ffi::WSAOVERLAPPED>,
    token: Option<usize>,
    pos: usize,
    operations: LinkedList<ffi::Operation>,
}

// On Windows we need to be careful when using IOCP on a server. Since we're "lending"
// access to the OS over memory we crate (we're not giving over ownership,
// but can't touch while it's lent either),
// it's easy to exploit this by issuing a lot of requests while delaying our
// responses. By doing this we would force the server to hand over so many write
// read buffers while waiting for clients to respond that it might run out of memory.
// Now the way we would normally handle this is to have a counter and limit the
// number of outstandig buffers, queueing requests and only handle them when the
// counter is below the high water mark. The same goes for using unlimited timeouts.
// http://www.serverframework.com/asynchronousevents/2011/06/tcp-flow-control-and-asynchronous-writes.html

impl TcpStream {
    pub fn connect(adr: impl net::ToSocketAddrs) -> io::Result<Self> {
        // This is a shortcut since this will block when establishing the connection.
        // There are several ways of avoiding this.
        // a) Obtrain the socket using system calls, set it to non_blocking before we connect
        // b) use the crate [net2](https://docs.rs/net2/0.2.33/net2/index.html) which
        // defines a trait with default implementation for TcpStream which allow us to set
        // it to non-blocking before we connect

        // Rust creates a WSASocket set to overlapped by default which is just what we need
        // https://github.com/rust-lang/rust/blob/f86521e0a33a2b54c4c23dbfc5250013f7a33b11/src/libstd/sys/windows/net.rs#L99
        let stream = net::TcpStream::connect(adr)?;
        stream.set_nonblocking(true)?;

        let mut buffer = vec![0_u8; 1024];
        let wsabuf = vec![ffi::WSABUF::new(buffer.len() as u32, buffer.as_mut_ptr())];
        Ok(TcpStream {
            inner: stream,
            buffer,
            wsabuf,
            event: None,
            token: None,
            pos: 0,
            operations: LinkedList::new(),
        })
    }
}

impl Read for TcpStream {
    fn read(&mut self, buff: &mut [u8]) -> io::Result<usize> {
        //   self.inner.read(buff)
        let mut bytes_read = 0;
        if self.buffer.len() - bytes_read <= buff.len() {
            for (a, b) in self.buffer.iter().skip(self.pos).zip(buff) {
                *b = *a;
                bytes_read += 1;
            }

            Ok(bytes_read)
        } else {
            for (b, a) in buff.iter_mut().zip(&self.buffer) {
                *b = *a;
                bytes_read += 1;
            }
            self.pos += bytes_read;
            Ok(bytes_read)
        }
    }
}

impl Write for TcpStream {
    fn write(&mut self, buff: &[u8]) -> io::Result<usize> {
        self.inner.write(buff)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl AsRawSocket for TcpStream {
    fn as_raw_socket(&self) -> RawSocket {
        self.inner.as_raw_socket()
    }
}

pub struct Registrator {
    completion_port: isize,
    is_poll_dead: Arc<AtomicBool>,
}

impl Registrator {
    pub fn register(
        &self,
        soc: &mut TcpStream,
        token: usize,
        interests: Interests,
    ) -> io::Result<()> {
        if self.is_poll_dead.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Poll instance is dead.",
            ));
        }

        ffi::create_io_completion_port(soc.as_raw_socket(), self.completion_port, 0)?;

        let op = ffi::Operation::new(token);
        soc.operations.push_back(op);

        if interests.is_readable() {
            ffi::wsa_recv(
                soc.as_raw_socket(),
                &mut soc.wsabuf,
                soc.operations.back_mut().unwrap(),
            )?;
        } else {
            unimplemented!();
        }

        Ok(())
    }

    pub fn close_loop(&self) -> io::Result<()> {
        if self
            .is_poll_dead
            .compare_and_swap(false, true, Ordering::SeqCst)
        {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Poll instance is dead.",
            ));
        }
        let mut overlapped = ffi::WSAOVERLAPPED::zeroed();
        ffi::post_queued_completion_status(self.completion_port, 0, 0, &mut overlapped)?;
        Ok(())
    }
}

// possible Arc<InnerSelector> needed
#[derive(Debug)]
pub struct Selector {
    completion_port: isize,
}

impl Selector {
    pub fn new() -> io::Result<Self> {
        // set up the queue
        let completion_port = ffi::create_completion_port()?;

        Ok(Selector { completion_port })
    }

    pub fn registrator(&self, is_poll_dead: Arc<AtomicBool>) -> Registrator {
        Registrator {
            completion_port: self.completion_port,
            is_poll_dead,
        }
    }

    /// Blocks until an Event has occured. Never times out. We could take a parameter
    /// for a timeout and pass it on but we'll not do that in our example.
    pub fn select(
        &mut self,
        events: &mut Vec<ffi::OVERLAPPED_ENTRY>,
        timeout: Option<i32>,
    ) -> io::Result<()> {
        // calling GetQueueCompletionStatus will either return a handle to a "port" ready to read or
        // block if the queue is empty.

        // Windows want timeout as u32 so we cast it as such
        let timeout = timeout.map(|t| t as u32);

        // first let's clear events for any previous events and wait until we get som more
        events.clear();
        let ul_count = events.capacity() as u32;

        let removed_res = ffi::get_queued_completion_status_ex(
            self.completion_port as isize,
            events,
            ul_count,
            timeout,
            false,
        );

        // We need to handle the case that the "error" was a WAIT_TIMEOUT error.
        // the code for this error is 258 on Windows. We don't treat this as an error
        // but set the events returned to 0.
        // (i tried to do this in the `ffi` function but there was an error)
        let removed = match removed_res {
            Ok(n) => n,
            Err(ref e) if e.raw_os_error() == Some(258) => 0,
            Err(e) => return Err(e),
        };

        unsafe {
            events.set_len(removed as usize);
        }

        Ok(())
    }
}

impl Drop for Selector {
    fn drop(&mut self) {
        match ffi::close_handle(self.completion_port) {
            Ok(_) => (),
            Err(e) => {
                if !std::thread::panicking() {
                    panic!(e);
                }
            }
        }
    }
}

mod ffi {
    use super::*;
    use std::io;
    use std::os::windows::io::RawSocket;
    use std::ptr;

    #[repr(C)]
    #[derive(Clone, Debug)]
    pub struct WSABUF {
        len: u32,
        buf: *mut u8,
    }

    impl WSABUF {
        pub fn new(len: u32, buf: *mut u8) -> Self {
            WSABUF { len, buf }
        }
    }

    #[repr(C)]
    #[derive(Debug, Clone)]
    pub struct OVERLAPPED_ENTRY {
        lp_completion_key: *mut usize,
        lp_overlapped: *mut WSAOVERLAPPED,
        internal: usize,
        bytes_transferred: u32,
    }

    impl OVERLAPPED_ENTRY {
        pub fn id(&self) -> Token {
            // TODO: this might be solvable wihtout sacrifising so much of Rust safety guarantees
            let operation: &Operation = unsafe { &*(self.lp_overlapped as *const Operation) };
            operation.token
        }

        pub(crate) fn zeroed() -> Self {
            OVERLAPPED_ENTRY {
                lp_completion_key: ptr::null_mut(),
                lp_overlapped: ptr::null_mut(),
                internal: 0,
                bytes_transferred: 0,
            }
        }
    }

    // Reference: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/ns-winsock2-wsaoverlapped
    #[repr(C)]
    #[derive(Debug)]
    pub struct WSAOVERLAPPED {
        /// Reserved for internal use
        internal: ULONG_PTR,
        /// Reserved
        internal_high: ULONG_PTR,
        /// Reserved for service providers
        offset: DWORD,
        /// Reserved for service providers
        offset_high: DWORD,
        /// If an overlapped I/O operation is issued without an I/O completion routine
        /// (the operation's lpCompletionRoutine parameter is set to null), then this parameter
        /// should either contain a valid handle to a WSAEVENT object or be null. If the
        /// lpCompletionRoutine parameter of the call is non-null then applications are free
        /// to use this parameter as necessary.
        h_event: HANDLE,
    }

    impl WSAOVERLAPPED {
        pub fn zeroed() -> Self {
            WSAOVERLAPPED {
                internal: ptr::null_mut(),
                internal_high: ptr::null_mut(),
                offset: 0,
                offset_high: 0,
                h_event: 0,
            }
        }
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct Operation {
        wsaoverlapped: WSAOVERLAPPED,
        token: usize,
    }

    impl Operation {
        pub(crate) fn new(token: usize) -> Self {
            Operation {
                wsaoverlapped: WSAOVERLAPPED::zeroed(),
                token,
            }
        }
    }

    // You can find most of these here: https://docs.microsoft.com/en-us/windows/win32/winprog/windows-data-types
    /// The HANDLE type is actually a `*mut c_void` but windows preserves backwards compatibility by allowing
    /// a INVALID_HANDLE_VALUE which is `-1`. We can't express that in Rust so it's much easier for us to treat
    /// this as an isize instead;
    pub type HANDLE = isize;
    pub type BOOL = bool;
    pub type WORD = u16;
    pub type DWORD = u32;
    pub type ULONG = u32;
    pub type PULONG = *mut ULONG;
    pub type ULONG_PTR = *mut usize;
    pub type PULONG_PTR = *mut ULONG_PTR;
    pub type LPDWORD = *mut DWORD;
    pub type LPWSABUF = *mut WSABUF;
    pub type LPWSAOVERLAPPED = *mut WSAOVERLAPPED;
    pub type LPWSAOVERLAPPED_COMPLETION_ROUTINE = *const extern "C" fn();

    // https://referencesource.microsoft.com/#System.Runtime.Remoting/channels/ipc/win32namedpipes.cs,edc09ced20442fea,references
    // read this! https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    /// Defined in `win32.h` which you can find on your windows system
    pub const INVALID_HANDLE_VALUE: HANDLE = -1;

    // https://docs.microsoft.com/en-us/windows/win32/winsock/windows-sockets-error-codes-2
    pub const WSA_IO_PENDING: i32 = 997;

    // This can also be written as `4294967295` if you look at sources on the internet.
    // Interpreted as an i32 the value is -1
    // see for yourself: https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=4b93de7d7eb43fa9cd7f5b60933d8935
    pub const INFINITE: u32 = 0xFFFFFFFF;

    #[link(name = "Kernel32")]
    extern "stdcall" {

        // https://docs.microsoft.com/en-us/windows/win32/fileio/createiocompletionport
        fn CreateIoCompletionPort(
            filehandle: HANDLE,
            existing_completionport: HANDLE,
            completion_key: ULONG_PTR,
            number_of_concurrent_threads: DWORD,
        ) -> HANDLE;
        // https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsarecv
        fn WSARecv(
            s: RawSocket,
            lpBuffers: LPWSABUF,
            dwBufferCount: DWORD,
            lpNumberOfBytesRecvd: LPDWORD,
            lpFlags: LPDWORD,
            lpOverlapped: LPWSAOVERLAPPED,
            lpCompletionRoutine: LPWSAOVERLAPPED_COMPLETION_ROUTINE,
        ) -> i32;
        // https://docs.microsoft.com/en-us/windows/win32/fileio/postqueuedcompletionstatus
        fn PostQueuedCompletionStatus(
            CompletionPort: HANDLE,
            dwNumberOfBytesTransferred: DWORD,
            dwCompletionKey: ULONG_PTR,
            lpOverlapped: LPWSAOVERLAPPED,
        ) -> i32;
        /// https://docs.microsoft.com/nb-no/windows/win32/api/ioapiset/nf-ioapiset-getqueuedcompletionstatus
        /// Errors: https://docs.microsoft.com/nb-no/windows/win32/debug/system-error-codes--0-499-
        /// From this we can see that error `WAIT_TIMEOUT` has the code 258 which we'll
        /// need later on
        fn GetQueuedCompletionStatusEx(
            CompletionPort: HANDLE,
            lpCompletionPortEntries: *mut OVERLAPPED_ENTRY,
            ulCount: ULONG,
            ulNumEntriesRemoved: PULONG,
            dwMilliseconds: DWORD,
            fAlertable: BOOL,
        ) -> i32;

        fn GetQueuedCompletionStatus(
            CompletionPort: HANDLE,
            lpNumberOfBytesTransferred: LPDWORD,
            lpCompletionKey: PULONG_PTR,
            lpOverlapped: LPWSAOVERLAPPED,
            dwMilliseconds: DWORD,
        ) -> i32;

        // https://docs.microsoft.com/nb-no/windows/win32/api/handleapi/nf-handleapi-closehandle
        fn CloseHandle(hObject: HANDLE) -> i32;

        // https://docs.microsoft.com/nb-no/windows/win32/api/winsock/nf-winsock-wsagetlasterror
        fn WSAGetLastError() -> i32;
    }

    // ===== SAFE WRAPPERS =====

    pub fn close_handle(handle: isize) -> io::Result<()> {
        let res = unsafe { CloseHandle(handle) };

        if res == 0 {
            Err(std::io::Error::last_os_error().into())
        } else {
            Ok(())
        }
    }

    pub fn create_completion_port() -> io::Result<isize> {
        unsafe {
            // number_of_concurrent_threads = 0 means use the number of physical threads but the argument is
            // ignored when existing_completionport is set to null.
            let res = CreateIoCompletionPort(INVALID_HANDLE_VALUE, 0, ptr::null_mut(), 0);
            if (res as *mut usize).is_null() {
                return Err(std::io::Error::last_os_error());
            }

            Ok(res)
        }
    }

    /// Returns the file handle to the completion port we passed in
    pub fn create_io_completion_port(
        s: RawSocket,
        completion_port: isize,
        token: usize,
    ) -> io::Result<isize> {
        let res =
            unsafe { CreateIoCompletionPort(s as isize, completion_port, token as *mut usize, 0) };

        if (res as *mut usize).is_null() {
            return Err(std::io::Error::last_os_error());
        }

        Ok(res)
    }

    /// Creates a socket read event.
    /// ## Returns
    /// The number of bytes recieved
    pub fn wsa_recv(
        s: RawSocket,
        wsabuffers: &mut [WSABUF],
        op: &mut Operation,
    ) -> Result<(), io::Error> {
        let mut flags = 0;
        let operation_ptr: *mut Operation = op;

        let res = unsafe {
            WSARecv(
                s,
                wsabuffers.as_mut_ptr(),
                1,
                ptr::null_mut(),
                &mut flags,
                operation_ptr as *mut WSAOVERLAPPED,
                ptr::null_mut(),
            )
        };
        if res != 0 {
            let err = unsafe { WSAGetLastError() };
            if err == WSA_IO_PENDING {
                // Everything is OK, and we can wait this with GetQueuedCompletionStatus
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        } else {
            // The socket is already ready so we don't need to queue it
            // TODO: Avoid queueing this
            Ok(())
        }
    }

    pub fn post_queued_completion_status(
        completion_port: isize,
        bytes_to_transfer: u32,
        completion_key: usize,
        overlapped_ptr: &mut WSAOVERLAPPED,
    ) -> io::Result<()> {
        let res = unsafe {
            PostQueuedCompletionStatus(
                completion_port,
                bytes_to_transfer,
                completion_key as *mut usize,
                overlapped_ptr,
            )
        };
        if res == 0 {
            Err(std::io::Error::last_os_error().into())
        } else {
            Ok(())
        }
    }

    /// ## Parameters:
    /// - *completion_port:* the handle to a completion port created by calling CreateIoCompletionPort
    /// - *completion_port_entries:* a pointer to an array of OVERLAPPED_ENTRY structures
    /// - *ul_count:* The maximum number of entries to remove
    /// - *timeout:* The timeout in milliseconds, if set to NONE, timeout is set to INFINITE
    /// - *alertable:* If this parameter is FALSE, the function does not return until the time-out period has elapsed or
    /// an entry is retrieved. If the parameter is TRUE and there are no available entries, the function performs
    /// an alertable wait. The thread returns when the system queues an I/O completion routine or APC to the thread
    /// and the thread executes the function.
    ///
    /// ## Returns
    /// The number of items actually removed from the queue
    pub fn get_queued_completion_status_ex(
        completion_port: isize,
        completion_port_entries: &mut [OVERLAPPED_ENTRY],
        ul_count: u32,
        timeout: Option<u32>,
        alertable: bool,
    ) -> io::Result<u32> {
        let mut ul_num_entries_removed: u32 = 0;
        // can't coerce directly to *mut *mut usize and cant cast `&mut` as `*mut`
        // let completion_key_ptr: *mut &mut usize = completion_key_ptr;
        // // but we can cast a `*mut ...`
        // let completion_key_ptr: *mut *mut usize = completion_key_ptr as *mut *mut usize;
        let timeout = timeout.unwrap_or(INFINITE);
        let res = unsafe {
            GetQueuedCompletionStatusEx(
                completion_port,
                completion_port_entries.as_mut_ptr(),
                ul_count,
                &mut ul_num_entries_removed,
                timeout,
                alertable,
            )
        };

        if res == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(ul_num_entries_removed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_new_creates_valid_port() {
        let selector = Selector::new().expect("create completion port failed");
        assert!(selector.completion_port > 0);
    }

    #[test]
    fn selector_register() {
        let selector = Selector::new().expect("create completion port failed");
        let poll_is_alive = Arc::new(AtomicBool::new(false));
        let registrator = selector.registrator(poll_is_alive.clone());
        let mut sock: TcpStream = TcpStream::connect("slowwly.robertomurray.co.uk:80").unwrap();
        let request = "GET /delay/1000/url/http://www.google.com HTTP/1.1\r\n\
                       Host: slowwly.robertomurray.co.uk\r\n\
                       Connection: close\r\n\
                       \r\n";
        sock.write_all(request.as_bytes())
            .expect("Error writing to stream");

        registrator
            .register(&mut sock, 1, Interests::READABLE)
            .expect("Error registering sock read event");
    }

    #[test]
    fn selector_select() {
        let mut selector = Selector::new().expect("create completion port failed");
        let poll_is_alive = Arc::new(AtomicBool::new(false));
        let registrator = selector.registrator(poll_is_alive.clone());
        let mut sock: TcpStream = TcpStream::connect("slowwly.robertomurray.co.uk:80").unwrap();
        let request = "GET /delay/1000/url/http://www.google.com HTTP/1.1\r\n\
                       Host: slowwly.robertomurray.co.uk\r\n\
                       Connection: close\r\n\
                       \r\n";
        sock.write_all(request.as_bytes())
            .expect("Error writing to stream");

        registrator
            .register(&mut sock, 2, Interests::READABLE)
            .expect("Error registering sock read event");
        let entry = ffi::OVERLAPPED_ENTRY::zeroed();
        let mut events: Vec<ffi::OVERLAPPED_ENTRY> = vec![entry; 255];
        selector.select(&mut events, None).expect("Select failed");

        for event in events {
            println!("COMPL_KEY: {:?}", event.id());
            assert_eq!(2, event.id());
        }

        println!("SOCKET AFTER EVENT RETURN: {:?}", sock);

        let mut buffer = String::new();
        sock.read_to_string(&mut buffer).unwrap();
        println!("BUFFERS: {}", buffer);
        assert!(!buffer.is_empty())
    }
}

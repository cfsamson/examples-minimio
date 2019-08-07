use std::error;
use std::fmt;
use std::io::Read;

#[cfg(target_os = "windows")]
pub use windows::{Event, EventLoop, EventResult};
//#[cfg(target_os="linux")]
//pub use linux::{Event, EventLoop, EventResult};

const MAXEVENTS: usize = 1000;

pub enum PollStatus<T: Read> {
    WouldBlock,
    Ready(Read),
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

mod windows {
    use crate::ElErr;
    use std::os::windows::io::{AsRawSocket, RawSocket};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::thread;

    use super::MAXEVENTS;

    pub struct Event<T> {
        /// Must not be moved, Windows holds raw pointers to this data
        data: T,
        id: usize,
        handle: Option<isize>,
        /// Must not be moved, Windows holds raw pointers to this data
        wsa_buffers: Vec<ffi::WSABUF>,

        /// Must not be moved, Windows holds raw pointers to this data
        bytes_recived: u32,

    }

    impl Event<String> {
        pub fn new_get(url: &str) -> Self {
            Event {
                data: String::new(),
                id: 0,
                handle: None,
                wsa_buffers: vec![],
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
        queue_handle: i32,
    }

    impl EventLoop {
        pub fn new() -> Result<Self, ElErr> {
            // set up the queue
            let queue_handle = ffi::create_queue()?;

            let has_error = Arc::new(AtomicUsize::new(0));
            let errors = Arc::new(Mutex::new(vec![]));

            let (loop_event_tx, loop_event_rx) = channel::<ffi::IOCPevent>();

            let errors_cloned = errors.clone();
            let has_error_clone = has_error.clone();
            thread::spawn(move || {
                let events = vec![ffi::IOCPevent::default(); MAXEVENTS];
                loop {
                    // TODO: wait for events

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

            Ok(EventLoop {
                has_error,
                errors,
                queue_handle,
            })
        }

        pub fn register_soc_read_event<T>(&mut self, soc: RawSocket) {}

        pub fn poll<T>(&mut self) -> Option<Vec<Event<T>>> {
            // calling GetQueueCompletionStatus will either return a handle to a "port" ready to read or
            // block if the queue is empty.
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

    pub enum EventResult {}

    mod ffi {
        use crate::ElErr;
        use std::os::raw::c_void;
        use std::os::windows::io::RawSocket;
        use std::ptr;

        #[derive(Debug, Clone)]
        pub struct IOCPevent {}

        impl Default for IOCPevent {
            fn default() -> Self {
                IOCPevent {}
            }
        }

        #[repr(C)]
        struct WSABUF {
            len: u32,
            buf: *mut u8,
        }

        impl WSABUF {
            fn new(len: u32, buf: *mut u8) -> Self {
                WSABUF {
                    len,
                    buf,
                }
            }
        }

        // Reference: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/ns-winsock2-wsaoverlapped
        #[repr(C)]
        struct WSAOVERLAPPED {
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

        // https://docs.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-overlapped
        struct OVERLAPPED {
            internal: ULONG_PTR,
            internal_high: ULONG_PTR,
            dummy: [DWORD; 2],
            h_event: HANDLE,
        }

        // You can find most of these here: https://docs.microsoft.com/en-us/windows/win32/winprog/windows-data-types
        /// The HANDLE type is actually a `*mut c_void` but windows preserves backwards compatibility by allowing
        /// a INVALID_HANDLE_VALUE which is `-1`. We can't express that in Rust so it's much easier for us to treat
        /// this as an isize instead;
        type HANDLE = isize;
        type DWORD = u32;
        type ULONG_PTR = *mut usize;
        type PULONG_PTR = *mut ULONG_PTR;
        type LPDWORD = *mut DWORD;
        type LPWSABUF = *mut WSABUF;
        type LPWSAOVERLAPPED = *mut WSAOVERLAPPED;
        type LPOVERLAPPED = *mut OVERLAPPED;

        // https://referencesource.microsoft.com/#System.Runtime.Remoting/channels/ipc/win32namedpipes.cs,edc09ced20442fea,references
        // read this! https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
        /// Defined in `win32.h` which you can find on your windows system
        static INVALID_HANDLE_VALUE: HANDLE = -1;

        // https://docs.microsoft.com/en-us/windows/win32/winsock/windows-sockets-error-codes-2
        static WSA_IO_PENDING: i32 = 997;

        // Funnily enough this is the same as -1 when interpreted as an i32
        // see for yourself: https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=cdb33e88acd34ef46bc052d427854210
        static INFINITE: u32 = 4294967295;

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
                lpFlags: DWORD,
                lpOverlapped: LPWSAOVERLAPPED,
            ) -> i32;
            // https://docs.microsoft.com/en-us/windows/win32/fileio/postqueuedcompletionstatus
            fn PostQueuedCompletionStatus(
                CompletionPort: HANDLE,
                dwNumberOfBytesTransferred: DWORD,
                dwCompletionKey: ULONG_PTR,
                lpOverlapped: LPWSAOVERLAPPED,
            ) -> i32;
            // https://docs.microsoft.com/nb-no/windows/win32/api/ioapiset/nf-ioapiset-getqueuedcompletionstatus
            fn GetQueuedCompletionStatus(
                CompletionPort: HANDLE,
                lpNumberOfBytesTransferred: LPDWORD,
                lpCompletionKey: PULONG_PTR,
                lpOverlapped: LPOVERLAPPED,
                dwMilliseconds: DWORD,
            ) -> i32;
            // https://docs.microsoft.com/nb-no/windows/win32/api/handleapi/nf-handleapi-closehandle
            fn CloseHandle(hObject: HANDLE) -> i32;

            // https://docs.microsoft.com/nb-no/windows/win32/api/winsock/nf-winsock-wsagetlasterror
            fn WSAGetLastError() -> i32;
        }

        // ===== SAFE WRAPPERS =====

        pub fn create_queue() -> Result<i32, ElErr> {
            unsafe {
                // number_of_concurrent_threads = 0 means use the number of physical threads but the argument is
                // ignored when existing_completionport is set to null.
                let res = CreateIoCompletionPort(INVALID_HANDLE_VALUE, 0, 0, 0);
                if (res as *mut usize).is_null() {
                    return Err(std::io::Error::last_os_error().into());
                }
                Ok(*(res as *const i32))
            }
        }

        pub fn create_soc_read_event(s: RawSocket, wsabuffers: &mut [WSABUF], bytes_recieved: &mut u32, ol: &mut WSAOVERLAPPED) -> Result<(), ElErr> {
            // This actually takes an array of buffers but we will only need one so we can just box it
            // and point to it (there is no difference in memory between a `vec![T; 1]` and a `Box::new(T)`)
            let buff_ptr: *mut WSABUF = wsabuffers.as_mut_ptr();
            //let num_bytes_recived_ptr: *mut u32 = bytes_recieved;

       
                let res = unsafe { WSARecv(s, buff_ptr, 1, bytes_recieved, 0, ol) };

                    if res != 0 {
                    let err = unsafe { WSAGetLastError() };

                    if err == WSA_IO_PENDING {
                        // Everything is OK, and we can wait this with GetQueuedCompletionStatus
                        Ok(())
                    } else {
                        return Err(std::io::Error::last_os_error().into());
                    }

                } else {
                    // The socket is already ready so we don't need to queue it
                    // TODO: Avoid queueing this
                    Ok(())
                }
            }

        pub fn register_event(completion_port: isize, bytes_to_transfer: u32, completion_key: &mut usize, overlapped_ptr: &mut WSAOVERLAPPED) -> Result<(), ElErr> {
            let res = unsafe { PostQueuedCompletionStatus(completion_port, bytes_to_transfer, completion_key, overlapped_ptr)};
            if res != 0 {
                Err(std::io::Error::last_os_error().into())
            } else {
                Ok(())
            }
        }


        pub fn get_queued_completion_status(completion_port: isize, bytes_transferred_ptr: &mut u32, completion_key_ptr: &mut &mut usize, overlapped_ptr: *mut OVERLAPPED) -> Result<(), ElErr> {
            // can't coerce directly to *mut *mut usize and cant cast `&mut` as `*mut`
            let completion_key_ptr: *mut &mut usize = completion_key_ptr;
            // but we can cast a `*mut ...`
            let completion_key_ptr: *mut *mut usize = completion_key_ptr as *mut *mut usize;
            let res = unsafe { GetQueuedCompletionStatus(completion_port, bytes_transferred_ptr, completion_key_ptr, overlapped_ptr, INFINITE)};

            if res != 0 {
                Err(std::io::Error::last_os_error().into())
            } else {
                Ok(())
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            #[test]
            fn create_queue_works() {
                let queue = create_queue().unwrap();
                assert!(queue > 0);
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

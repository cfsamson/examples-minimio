

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

            EventLoop {
                has_error,
                errors,
            }
        }

        pub fn register_soc_read_event<T>(&mut self, soc: RawSocket) {
            
        }

        pub fn poll<T>(&mut self) -> Option<Vec<Event<T>>> {
            // calling GetQueueCompletionStatus wil either return a handle to a "port" ready to read or 
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

    pub enum EventResult {

    }

    mod ffi {
        use std::ptr;
        use std::os::raw::{c_void, c_int};
        use std::os::windows::io::{AsRawSocket, RawSocket};
        #[derive(Debug, Clone)]
        pub struct IOCPevent {

        }

        impl Default for IOCPevent {
            fn default() -> Self {
                IOCPevent {

                }
            }
        }

        const INVALID_HANDLE_VALUE: i32 = -1;

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

        https://docs.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-overlapped
        struct OVERLAPPED {
            internal: ULONG_PTR,
            internal_high: ULONG_PTR,
            dummy: [DWORD; 2],
            h_event: HANDLE,
        }

        // You can find most of these here: https://docs.microsoft.com/en-us/windows/win32/winprog/windows-data-types
        type HANDLE = *mut c_void;
        type DWORD = u32;
        type ULONG_PTR = usize;
        type PULONG_PTR = *mut ULONG_PTR;
        type LPDWORD = *mut DWORD;
        type LPWSABUF = *mut u8;
        type LPWSAOVERLAPPED  = *mut WSAOVERLAPPED;
        type LPOVERLAPPED = *mut OVERLAPPED;

        #[link(name = "win32")]
        extern "stdcall" {
            // https://referencesource.microsoft.com/#System.Runtime.Remoting/channels/ipc/win32namedpipes.cs,edc09ced20442fea,references
            //static INVALID_HANDLE_VALUE: c_int;
            // https://docs.microsoft.com/en-us/windows/win32/fileio/createiocompletionport
            fn CreateIoCompletionPort(filehandle: HANDLE, existing_completionport: HANDLE, completion_key: ULONG_PTR, number_of_concurrent_threads: DWORD) -> HANDLE;
            // https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsarecv
            fn WSARecv(s: RawSocket, lpBuffers: LPWSABUF, dwBufferCount: DWORD, lpNumberOfBytesRecvd: LPDWORD, lpFlags: DWORD, lpOverlapped: LPWSAOVERLAPPED) -> i32;
            // https://docs.microsoft.com/en-us/windows/win32/fileio/postqueuedcompletionstatus
            fn PostQueuedCompletionStatus(CompletionPort: HANDLE, dwNumberOfBytesTransferred: DWORD, dwCompletionKey: ULONG_PTR, lpOverlapped: LPWSAOVERLAPPED) -> i32;
            // https://docs.microsoft.com/nb-no/windows/win32/api/ioapiset/nf-ioapiset-getqueuedcompletionstatus
            fn GetQueuedCompletionStatus(CompletionPort: HANDLE, lpNumberOfBytesTransferred: LPDWORD, lpCompletionKey: PULONG_PTR, lpOverlapped: OVERLAPPED, dwMilliseconds: DWORD) -> i32;
            // https://docs.microsoft.com/nb-no/windows/win32/api/handleapi/nf-handleapi-closehandle
            fn CloseHandle(hObject: HANDLE) -> i32;
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
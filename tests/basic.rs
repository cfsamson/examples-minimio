use minimio::{Poll, Events, TcpStream, Interests};
use std::io::{Read, Write};

#[test]
fn main() {
    let mut poll = Poll::new().unwrap(); //this is different form mio
    let mut events = Events::with_capacity(1024);

    let mut stream = TcpStream::connect("slowwly.robertomurray.co.uk:80").unwrap();

    let request = "GET /delay/1000/url/http://www.google.com HTTP/1.1\r\n\
                       Host: slowwly.robertomurray.co.uk\r\n\
                       Connection: close\r\n\
                       \r\n";
    stream.write_all(request.as_bytes()).expect("Error writing to stream");

    // We need a way to pass in an id as well
    let _ = poll.register_with_id(&mut stream, Interests::readable(), 100).unwrap();

       poll.poll(&mut events).unwrap();
       for event in &events {
           if event.token().unwrap().value() == 100 {
               // Socket connected (could be a spurious wakeup)
               let mut buffer = String::new();
               stream.read_to_string(&mut buffer).unwrap();
               println!("{}", buffer);
        
           }
       }

}

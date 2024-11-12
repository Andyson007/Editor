use std::{fs::File, io::BufReader, net::TcpListener, sync::Arc, thread::spawn};

use ropey::Rope;
use tungstenite::{
    accept_hdr,
    handshake::server::{Request, Response},
    Message,
};

fn main() {
    let server = TcpListener::bind("127.0.0.1:3012").unwrap();
    let file = File::open("../file.txt").unwrap();
    let rope = Arc::new(Rope::from_reader(BufReader::new(file)).unwrap());
    for stream in server.incoming() {
        let rope = rope.clone();
        spawn(move || {
            let callback = |req: &Request, mut response: Response| {
                println!("Received a new ws handshake");
                println!("The request's path is: {}", req.uri().path());
                println!("The request's headers are:");
                for (header, _value) in req.headers() {
                    println!("* {header}");
                }

                // Let's add an additional header to our response to the client.
                let headers = response.headers_mut();
                headers.append("MyCustomHeader", ":)".parse().unwrap());
                headers.append("SOME_TUNGSTENITE_HEADER", "header_value".parse().unwrap());

                Ok(response)
            };
            let mut websocket = accept_hdr(stream.unwrap(), callback).unwrap();
            websocket
                .send(Message::Binary(rope.bytes().collect()))
                .unwrap();
            loop {
                let msg = websocket.read().unwrap();
                if msg.is_binary() || msg.is_text() {
                    websocket.send(msg).unwrap();
                }
            }
        });
    }
}

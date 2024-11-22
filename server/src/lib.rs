use std::{
    fs::File,
    io::BufReader,
    net::TcpListener,
    sync::{Arc, RwLock},
    thread::spawn,
};

use btep::Btep;
use piece_table::Piece;
use tracing::{debug, error, info, trace, warn};
use tungstenite::{
    accept_hdr,
    handshake::server::{Request, Response},
    http::{self, StatusCode},
};

pub fn run() {
    let server = TcpListener::bind("127.0.0.1:3012").unwrap();
    let file = File::open("./file.txt").unwrap();
    let text = Arc::new(RwLock::new(
            Piece::original_from_reader(BufReader::new(file)).unwrap(),
    ));
    for stream in server.incoming() {
        let text = text.clone();
        spawn(move || {
            let callback = |req: &Request, response: Response| {
                debug!("Received new ws handshake");
                trace!("Received a new ws handshake");
                trace!("The request's path is: {}", req.uri().path());
                trace!("The request's headers are:");
                for (header, _value) in req.headers() {
                    trace!("* {header}");
                }

                if req.headers().get("test").is_some_and(|x| x == "test") {
                    Ok(response)
                } else {
                    Err(http::Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(None)
                        .unwrap())
                }
            };
            let mut websocket = accept_hdr(stream.unwrap(), callback).unwrap();
            {
                let data = text.read().unwrap();
                // dbg!(&data);
                websocket.send(Btep::Full(&*data).into_message()).unwrap();
            }
            loop {
                let msg = websocket.read().unwrap();
                if msg.is_binary() || msg.is_text() {
                    debug!("{msg:?}");
                }
            }
        });
    }
}

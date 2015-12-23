use std::net::{TcpListener, TcpStream};
use std::io::prelude::*;
use std::io::{Error, ErrorKind};
use std::thread;
use std::result;

type Result<T> = result::Result<T, &'static str>;

fn to_u32(values: &[u8]) -> u32 {
    let mut out = 0;
    for &i in values {
        out = out << 4 | i as u32;
    }
    out
}

fn read_u32(stream: &mut Read, buf: &mut Vec<u8>) -> std::io::Result<u32> {
    match stream.read(buf) {
        Ok(len) if len < 4 => Err(Error::new(ErrorKind::Other, "read_u32: not enough bytes")),
        Ok(_) => Ok(to_u32(buf)),
        Err(e) => Err(e)
    }
}

fn handle_client(mut stream: TcpStream) {
    loop {
        let mut buf = Vec::with_capacity(512);
        buf.resize(4, 0);
        
        let command_length = match read_u32(&mut stream, &mut buf) {
            Ok(v) => v,
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
        };

        println!("< command_length: {}", command_length);

        let pdu_total_len = command_length as usize;
        buf.resize(pdu_total_len, 0);

        match stream.read(&mut buf[4..]) {
            Ok(len) => {
                println!("! got additional {} byte(s)", len);
            }

            Err(e) => {
                println!("handle_client: oops! {}", e);
                break;
            }
        }
        
        for x in &buf {
            print!("{:x}", x);
        }
        
        println!("");
    }
}

fn main() {
    let listener = TcpListener::bind("0.0.0.0:1234").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    handle_client(stream);
                });
            }

            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }

    drop(listener);
}

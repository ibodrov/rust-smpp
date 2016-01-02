extern crate rustc_serialize;

use std::net::{TcpListener, TcpStream};
use std::io::prelude::*;
use std::thread;
use std::collections::HashMap;
use std::io::{BufReader, BufWriter};

#[derive(Debug)]
enum SmppError {
    InvalidMessageLength, // ESME_RINVMSGLEN
    InvalidCommandId      // ESME_RINVCMDID
}

#[derive(Debug)]
enum SmppCommand {
    BindTransceiver,
    BindTransceiverResp
}

impl SmppCommand {
    fn from_id(id: u32) -> Option<SmppCommand> {
        match id {
            0x00000009 => Some(SmppCommand::BindTransceiver),
            0x80000009 => Some(SmppCommand::BindTransceiverResp),
            _ => None
        }
    }

    fn to_id(self) -> Option<u32> {
        match self {
            SmppCommand::BindTransceiver => Some(0x00000009),
            SmppCommand::BindTransceiverResp => Some(0x80000009)
        }
    }
}

#[derive(Debug)]
enum HeaderValue {
    Str(String),
    Byte(u8)
}

#[derive(Debug)]
struct SmppMessage<'a> {
    command_length: u32,
    command: SmppCommand,
    command_status: u32,
    sequence_number: u32,

    headers: HashMap<&'a str, HeaderValue>
}

impl<'a> SmppMessage<'a> {
    fn make_resp(self: &SmppMessage<'a>, command_status: u32) -> SmppMessage<'a> {
        SmppMessage {
            command_length: 0,
            command: SmppCommand::BindTransceiverResp,
            command_status: command_status,
            sequence_number: self.sequence_number,
            headers: HashMap::new()
        }
    }
}

#[derive(Debug)]
enum SmppConnectionStatus {
    NotYetBound,
    Bound
}

struct SmppConnection {
    status: SmppConnectionStatus,
    stream: TcpStream
}

impl SmppConnection {
    fn new(stream: TcpStream) -> SmppConnection {
        SmppConnection {
            status: SmppConnectionStatus::NotYetBound,
            stream: stream
        }
    }

    fn read_pdu(r: &mut Iterator<Item=u8>) -> Result<SmppMessage, SmppError> {
        let command_length = try!(read_u32(r));
        let command = try!(SmppCommand::from_id(try!(read_u32(r)))
                           .ok_or(SmppError::InvalidCommandId));
        let command_status = try!(read_u32(r));
        let sequence_number = try!(read_u32(r));
        
        let mut headers = HashMap::new();
        match command {
            SmppCommand::BindTransceiver => {
                headers.insert("system_id", HeaderValue::Str(read_cstring(r)));
                headers.insert("password", HeaderValue::Str(read_cstring(r)));
                headers.insert("system_type", HeaderValue::Str(read_cstring(r)));
                headers.insert("interface_version", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("addr_ton", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("addr_npi", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("address_range", HeaderValue::Str(read_cstring(r)));
            }
            
            _ => return Err(SmppError::InvalidCommandId)
        };
        
        Ok(SmppMessage {
            command_length: command_length,
            command: command,
            command_status: command_status,
            sequence_number: sequence_number,
            headers: headers
        })
    }

    fn write_pdu(msg: SmppMessage, w: &mut Write) -> std::io::Result<usize> {
        let mut buf: Vec<u8> = Vec::with_capacity(512);
        let mut written = 0;
        
        try!(write_u32(&mut buf, msg.command.to_id().unwrap()));
        try!(write_u32(&mut buf, msg.command_status));
        try!(write_u32(&mut buf, msg.sequence_number));
        try!(buf.write(&vec![0u8]));
        
        written = written + write_u32(w, (buf.len() + 4) as u32).unwrap();
        written = written + w.write(&buf).unwrap();
        
        Ok(written)
    }
}

fn read_u32(r: &mut Iterator<Item=u8>) -> Result<u32, SmppError> {
    let mut out = 0;

    for _ in 0..4 {
        let v = match r.next() {
            Some(i) => i,
            None => return Err(SmppError::InvalidMessageLength)
        };

        out = out << 4 | v as u32;
    }

    Ok(out)
}

fn write_u32(w: &mut Write, i: u32) -> std::io::Result<usize> {
    println!("write_u32: {}", i);
    let ab = unsafe { std::mem::transmute::<u32, [u8; 4]>(i.to_be()) };
    w.write(&ab)
}

fn read_u8(r: &mut Iterator<Item=u8>) -> Result<u8, SmppError> {
    match r.next() {
        Some(i) => Ok(i),
        None => Err(SmppError::InvalidMessageLength)
    }
}

fn read_cstring(r: &mut Iterator<Item=u8>) -> String {
    r.take_while(|&c| c != 0)
        .map(|c| c as char)
        .collect::<String>()
}

fn handle_client(mut conn: SmppConnection) {
    // Result<u8> -> u8
    let flatten = |b| match b {
        Ok(value) => value,
        Err(e) => panic!("handle_client error: {}", e)
    };

    let stream = &conn.stream;
    let mut bytes = BufReader::new(stream).bytes().map(flatten);
    let mut writer = BufWriter::new(stream);

    loop {
        let pdu = SmppConnection::read_pdu(&mut bytes).unwrap();
        println!("<< got: {:?}", pdu);
        
        match pdu.command {
            SmppCommand::BindTransceiver => {
                let resp;
                
                match conn.status {
                    SmppConnectionStatus::NotYetBound => {
                        // ESME_ROK
                        resp = pdu.make_resp(0x00000000);
                        conn.status = SmppConnectionStatus::Bound;
                        println!("bound!");
                    },

                    _ => {
                        // ESME_RALYBND
                        resp = pdu.make_resp(0x00000005);
                    }
                }
                
                SmppConnection::write_pdu(resp, &mut writer).unwrap();
                writer.flush().unwrap();
            }

            _ => println!("nothing to do")
        }
    }
}

fn main() {
    let listener = TcpListener::bind("0.0.0.0:1234").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    let c = SmppConnection::new(stream);
                    handle_client(c);
                });
            }

            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }

    drop(listener);
}

use std::net::{TcpListener, TcpStream};
use std::io::prelude::*;
use std::thread;
use std::collections::HashMap;
use std::io::{BufReader, BufWriter};

#[derive(Debug)]
enum SmppError {
    EOF,
    UnsupportedCommandId,
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
    fn make_resp() -> SmppMessage<'a> {
        SmppMessage {
            command_length: 0,
            command: SmppCommand::BindTransceiverResp,
            command_status: 0,
            sequence_number: 0,
            headers: HashMap::new()
        }
    }
}

fn read_u32(r: &mut Iterator<Item=u8>) -> Result<u32, SmppError> {
    let mut out = 0;

    for _ in 0..4 {
        let v = match r.next() {
            Some(i) => i,
            None => return Err(SmppError::EOF)
        };
        
        out = out << 4 | v as u32;
    }
    
    Ok(out)
}

fn write_u32(w: &mut Write, i: u32) -> std::io::Result<usize> {
    let ab = [(i >> 24) as u8,
              (i >> 16) as u8,
              (i >> 8) as u8,
              i as u8];
    w.write(&ab)
}

fn read_u8(r: &mut Iterator<Item=u8>) -> Result<u8, SmppError> {
    match r.next() {
        Some(i) => Ok(i),
        None => Err(SmppError::EOF)
    }
}
    
fn read_cstring(r: &mut Iterator<Item=u8>) -> String {
    r.take_while(|&c| c != 0)
        .map(|c| c as char)
        .collect::<String>()
}

fn read_pdu(r: &mut Iterator<Item=u8>) -> Result<SmppMessage, SmppError> {
    let command_length = try!(read_u32(r));
    let command = try!(SmppCommand::from_id(try!(read_u32(r)))
                       .ok_or(SmppError::UnsupportedCommandId));
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

        _ => return Err(SmppError::UnsupportedCommandId)
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
    try!(buf.write("test".as_bytes()));

    written = written + write_u32(w, buf.len() as u32).unwrap();
    written = written + w.write(&buf).unwrap();

    Ok(written)
}

fn handle_client(stream: TcpStream) {
    let flatten = |b| match b {
        Ok(value) => value,
        Err(e) => panic!("handle_client error: {}", e)
    };

    let mut bytes = BufReader::new(&stream).bytes()
        .map(flatten);

    let mut writer = BufWriter::new(&stream);
    
    loop {
        let pdu = read_pdu(&mut bytes).unwrap();
        match pdu.command {
            SmppCommand::BindTransceiver => {
                let resp = SmppMessage::make_resp();
                write_pdu(resp, &mut writer).unwrap();
                writer.flush().unwrap();
            }

            _ => println!("nothing to do")
        }
        println!("<< got: {:?}", pdu);
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

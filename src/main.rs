mod smpp;

use std::net::{TcpListener, TcpStream};
use std::io::prelude::*;
use std::thread;
use std::collections::HashMap;
use std::io::{BufReader, BufWriter};

use smpp::{SmppCommandStatus, SmppCommand, HeaderValue, SmppMessage};


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

    fn read_pdu(r: &mut Iterator<Item=u8>) -> Result<SmppMessage, SmppCommandStatus> {
        let command_length = try!(read_u32(r));
        let command = try!(SmppCommand::from_id(try!(read_u32(r)))
                           .ok_or(SmppCommandStatus::InvalidCommandId));
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

            SmppCommand::EnquireLink => {
                // NOOP
            }

            SmppCommand::SubmitSm => {
                headers.insert("service_type", HeaderValue::Str(read_cstring(r)));
                headers.insert("source_addr_ton", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("source_addr_npi", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("source_addr", HeaderValue::Str(read_cstring(r)));
                headers.insert("dest_addr_ton", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("dest_addr_npi", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("destination_addr", HeaderValue::Str(read_cstring(r)));
                headers.insert("esm_class", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("protocol_id", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("priority_flag", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("schedule_delivery_time", HeaderValue::Str(read_cstring(r)));
                headers.insert("validity_period", HeaderValue::Str(read_cstring(r)));
                headers.insert("registered_delivery", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("replace_if_present", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("data_coding", HeaderValue::Byte(read_u8(r).unwrap()));
                headers.insert("sm_default_msg_id", HeaderValue::Byte(read_u8(r).unwrap()));

                let sm_length = read_u8(r).unwrap();
                headers.insert("sm_length", HeaderValue::Byte(sm_length));
                headers.insert("short_message", HeaderValue::ByteArray(read_exact(r, sm_length as usize)));
            }

            SmppCommand::Unbind => {
                // NOOP
            }

            _ => return Err(SmppCommandStatus::InvalidCommandId)
        };

        Ok(SmppMessage::new(command_length,
                            command,
                            command_status,
                            sequence_number,
                            headers))
    }

    fn write_pdu(msg: SmppMessage, w: &mut Write) -> std::io::Result<usize> {
        let mut buf: Vec<u8> = Vec::with_capacity(512);
        let mut written = 0;

        // command_id
        let command_id = msg.command().to_id();
        try!(write_u32(&mut buf, command_id));

        // command_status
        try!(write_u32(&mut buf, msg.command_status()));

        // sequence_number
        try!(write_u32(&mut buf, msg.sequence_number()));

        // body
        match *msg.command() {
            SmppCommand::BindTransceiverResp => {
                try!(buf.write(&vec![0u8])); // system_id
            }

            SmppCommand::EnquireLinkResp => {
                // NOOP
            }

            SmppCommand::SubmitSmResp => {
                if msg.command_status() == 0 {
                    let message_id = msg.get_str("message_id").as_bytes();
                    try!(buf.write(&message_id));
                    try!(buf.write(&vec![0u8]));
                }
            }

            SmppCommand::UnbindResp => {
                // NOOP
            }

            _ => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "write_pdu: Unsupported command"))
        }

        // calculate the size of the pdu...
        let len = (buf.len() + 4) as u32;
        // ...and write it in the first 4 octets of the output
        written = written + write_u32(w, len).unwrap();
        // write rest of the pdu
        written = written + w.write(&buf).unwrap();

        Ok(written)
    }
}

/// Reads an u32 value from the specified iterator.
fn read_u32(r: &mut Iterator<Item=u8>) -> Result<u32, SmppCommandStatus> {
    let mut out = 0;

    for _ in 0..4 {
        let v = match r.next() {
            Some(i) => i,
            None => return Err(SmppCommandStatus::InvalidMessageLength)
        };

        out = out << 4 | v as u32;
    }

    Ok(out)
}

fn write_u32(w: &mut Write, i: u32) -> std::io::Result<usize> {
    let ab = unsafe { std::mem::transmute::<u32, [u8; 4]>(i.to_be()) };
    w.write(&ab)
}

fn read_u8(r: &mut Iterator<Item=u8>) -> Result<u8, SmppCommandStatus> {
    match r.next() {
        Some(i) => Ok(i),
        None => Err(SmppCommandStatus::InvalidMessageLength)
    }
}

fn read_cstring(r: &mut Iterator<Item=u8>) -> String {
    r.take_while(|&c| c != 0)
        .map(|c| c as char)
        .collect::<String>()
}

fn read_exact(r: &mut Iterator<Item=u8>, len: usize) -> Vec<u8> {
    r.take(len).collect()
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
        //println!("<< got: {:?}", pdu);

        match *pdu.command() {
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

                let resp = resp.unwrap(); // TODO
                SmppConnection::write_pdu(resp, &mut writer).unwrap();
                writer.flush().unwrap();
            }

            SmppCommand::EnquireLink => {
                let resp = pdu.make_resp(0x00000000).unwrap();
                SmppConnection::write_pdu(resp, &mut writer).unwrap();
                writer.flush().unwrap();
                //println!("enquire_link_resp!");
            }

            SmppCommand::SubmitSm => {
                let mut resp = pdu.make_resp(0x00000000).unwrap();
                resp.set_str("message_id", pdu.sequence_number().to_string());
                SmppConnection::write_pdu(resp, &mut writer).unwrap();
                writer.flush().unwrap();
                //println!("submit_sm_resp!");
            }

            SmppCommand::Unbind => {
                let resp = pdu.make_resp(0x00000000).unwrap();
                SmppConnection::write_pdu(resp, &mut writer).unwrap();
                writer.flush().unwrap();
                //println!("unbind_resp!");
                return;
            }

            _ => {
                //println!("nothing to do")
            }
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

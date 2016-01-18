use std::mem;
use std::io;
use smpp::SmppCommandStatus;

pub fn write_u32(w: &mut io::Write, i: u32) -> io::Result<usize> {
    let ab = unsafe { mem::transmute::<u32, [u8; 4]>(i.to_be()) };
    w.write(&ab)
}

/// Reads an u32 value from the specified iterator.
pub fn read_u32(r: &mut Iterator<Item=u8>) -> Result<u32, SmppCommandStatus> {
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

pub fn read_u8(r: &mut Iterator<Item=u8>) -> Result<u8, SmppCommandStatus> {
    match r.next() {
        Some(i) => Ok(i),
        None => Err(SmppCommandStatus::InvalidMessageLength)
    }
}

pub fn read_cstring(r: &mut Iterator<Item=u8>) -> String {
    r.take_while(|&c| c != 0)
        .map(|c| c as char)
        .collect::<String>()
}

pub fn read_exact(r: &mut Iterator<Item=u8>, len: usize) -> Vec<u8> {
    r.take(len).collect()
}

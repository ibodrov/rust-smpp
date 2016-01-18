use std::collections::HashMap;

#[derive(Debug)]
pub enum SmppCommandStatus {
    /// ESME_RINVMSGLEN
    InvalidMessageLength,

    // ESME_RINVCMDID
    InvalidCommandId
}

#[derive(Debug)]
pub enum SmppCommand {
    SubmitSm,
    SubmitSmResp,
    Unbind,
    UnbindResp,
    BindTransceiver,
    BindTransceiverResp,
    EnquireLink,
    EnquireLinkResp,
}

impl SmppCommand {
    pub fn from_id(id: u32) -> Option<SmppCommand> {
        match id {
            0x00000004 => Some(SmppCommand::SubmitSm),
            0x80000004 => Some(SmppCommand::SubmitSmResp),
            0x00000006 => Some(SmppCommand::Unbind),
            0x80000006 => Some(SmppCommand::UnbindResp),
            0x00000009 => Some(SmppCommand::BindTransceiver),
            0x80000009 => Some(SmppCommand::BindTransceiverResp),
            0x00000015 => Some(SmppCommand::EnquireLink),
            0x80000015 => Some(SmppCommand::EnquireLinkResp),
            _ => None,
        }
    }

    pub fn to_id(&self) -> u32 {
        match *self {
            SmppCommand::SubmitSm => 0x00000004,
            SmppCommand::SubmitSmResp => 0x80000004,
            SmppCommand::Unbind => 0x00000006,
            SmppCommand::UnbindResp => 0x80000006,
            SmppCommand::BindTransceiver => 0x00000009,
            SmppCommand::BindTransceiverResp => 0x80000009,
            SmppCommand::EnquireLink => 0x00000015,
            SmppCommand::EnquireLinkResp => 0x80000015,
        }
    }
}

#[derive(Debug)]
pub enum HeaderValue {
    Str(String),
    Byte(u8),
    ByteArray(Vec<u8>),
}

#[derive(Debug)]
pub struct SmppMessage<'a> {
    command_length: u32,
    command: SmppCommand,
    command_status: u32,
    sequence_number: u32,

    headers: HashMap<&'a str, HeaderValue>,
}

impl<'a> SmppMessage<'a> {
    pub fn new(command_length: u32, command: SmppCommand, command_status: u32, sequence_number: u32, headers: HashMap<&'a str, HeaderValue>) -> SmppMessage<'a> {
        SmppMessage {
            command_length: command_length,
            command: command,
            command_status: command_status,
            sequence_number: sequence_number,
            headers: headers,
        }
    }

    pub fn command(&self) -> &SmppCommand {
        &self.command
    }

    pub fn command_status(&self) -> u32 {
        self.command_status
    }

    pub fn sequence_number(&self) -> u32 {
        self.sequence_number
    }

    pub fn make_resp(&self, command_status: u32) -> Result<SmppMessage<'a>, SmppCommandStatus> {
        let command = match self.command {
            SmppCommand::BindTransceiver => SmppCommand::BindTransceiverResp,
            SmppCommand::EnquireLink => SmppCommand::EnquireLinkResp,
            SmppCommand::SubmitSm => SmppCommand::SubmitSmResp,
            SmppCommand::Unbind => SmppCommand::UnbindResp,
            _ => return Err(SmppCommandStatus::InvalidCommandId),
        };

        Ok(SmppMessage {
            command_length: 0,
            command: command,
            command_status: command_status,
            sequence_number: self.sequence_number,
            headers: HashMap::new()
        })
    }

    pub fn get_str(&self, k: &str) -> &String {
        match self.headers.get(k) {
            Some(&HeaderValue::Str(ref x)) => x,
            _ => panic!("get_str: missing value or invalid type") // TODO more details
        }
    }

    pub fn set_str(&mut self, k : &'a str, v: String) {
        self.headers.insert(k, HeaderValue::Str(v));
    }
}

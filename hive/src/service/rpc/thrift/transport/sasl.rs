use super::PlainClient;
use byteorder::{BigEndian, ReadBytesExt as _, WriteBytesExt as _};
use std::io::{Read as _, Write as _};
use thrift::transport::{
    ReadHalf, TFramedReadTransport, TFramedWriteTransport, TIoChannel as _, TTcpChannel, WriteHalf,
};

pub type TSaslClientReadTransport = TFramedReadTransport<ReadHalf<TTcpChannel>>;
pub type TSaslClientWriteTransport = TFramedWriteTransport<WriteHalf<TTcpChannel>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NegotiationStatus {
    Start = 1,
    Ok,
    Bad,
    Error,
    Complete,
}

impl NegotiationStatus {
    fn name(&self) -> &str {
        match self {
            NegotiationStatus::Start => "START",
            NegotiationStatus::Ok => "OK",
            NegotiationStatus::Bad => "BAD",
            NegotiationStatus::Error => "ERROR",
            NegotiationStatus::Complete => "COMPLETE",
        }
    }
}

impl TryFrom<u8> for NegotiationStatus {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, anyhow::Error> {
        match value {
            1 => Ok(Self::Start),
            2 => Ok(Self::Ok),
            3 => Ok(Self::Bad),
            4 => Ok(Self::Error),
            5 => Ok(Self::Complete),
            _ => Err(anyhow::anyhow!("Invalid status {}", value)),
        }
    }
}

#[derive(Debug)]
struct SaslResponse {
    status: NegotiationStatus,
    payload: Vec<u8>,
}

#[derive(Debug)]
pub struct TSaslClientTransport {
    channel: TTcpChannel,
    mechanism: String,
    sasl_client: PlainClient,
}

impl TSaslClientTransport {
    pub fn new(channel: TTcpChannel, username: String, password: Vec<u8>) -> Self {
        let sasl_client = PlainClient::new(None, username, password);
        Self {
            channel,
            mechanism: sasl_client.mechanism_name().to_owned(),
            sasl_client,
        }
    }

    pub fn open(mut self) -> anyhow::Result<Self> {
        if self.sasl_client.is_complete() {
            Err(anyhow::anyhow!("SASL transport already open"))?
        }
        self.handle_sasl_start_message()?;
        let mut message = None;
        while !self.sasl_client.is_complete() {
            let _message = self.receive_sasl_message()?;
            match _message.status {
                NegotiationStatus::Ok => {
                    let response = self.sasl_client.step(&_message.payload)?;
                    self.send_sasl_message(
                        if self.sasl_client.is_complete() {
                            NegotiationStatus::Complete
                        } else {
                            NegotiationStatus::Ok
                        },
                        &response,
                    )?;
                }
                NegotiationStatus::Complete => (),
                _ => Err(anyhow::anyhow!(
                    "Expected COMPLETE or OK, got {}",
                    _message.status.name()
                ))?,
            }
            message = Some(_message);
        }
        if message.is_none() || message.is_some_and(|m| m.status == NegotiationStatus::Ok) {
            let _message = self.receive_sasl_message()?;
            if _message.status != NegotiationStatus::Complete {
                Err(anyhow::anyhow!(
                    "Expected SASL COMPLETE, but got {}",
                    _message.status.name()
                ))?
            }
        }
        Ok(self)
    }

    fn handle_sasl_start_message(&mut self) -> anyhow::Result<()> {
        let mut initial_response = vec![];
        if self.sasl_client.has_initial_response() {
            initial_response = self.sasl_client.step(&initial_response)?;
        }
        let mechanism = self.mechanism.to_owned();
        self.send_sasl_message(NegotiationStatus::Start, mechanism.as_bytes())?;
        self.send_sasl_message(
            if self.sasl_client.is_complete() {
                NegotiationStatus::Complete
            } else {
                NegotiationStatus::Ok
            },
            &initial_response,
        )?;
        self.channel.flush()?;
        Ok(())
    }

    fn send_sasl_message(
        &mut self,
        status: NegotiationStatus,
        payload: &[u8],
    ) -> anyhow::Result<()> {
        self.channel.write_u8(status as u8)?;
        self.channel.write_u32::<BigEndian>(payload.len() as u32)?;
        self.channel.write_all(payload)?;
        self.channel.flush()?;
        Ok(())
    }

    fn receive_sasl_message(&mut self) -> anyhow::Result<SaslResponse> {
        let status = NegotiationStatus::try_from(self.channel.read_u8()?)?;
        let payload_bytes = self.channel.read_i32::<BigEndian>()?;
        if !(0..=104857600).contains(&payload_bytes) {
            Err(anyhow::anyhow!(
                "Invalid payload header length: {}",
                payload_bytes
            ))?
        }
        let mut payload = vec![0; payload_bytes as usize];
        self.channel.read_exact(&mut payload)?;
        if ![NegotiationStatus::Bad, NegotiationStatus::Error].contains(&status) {
            Ok(SaslResponse { status, payload })
        } else {
            Err(anyhow::anyhow!(
                "Peer indicated failure: {}",
                String::from_utf8(payload)?
            ))?
        }
    }

    pub fn split(self) -> anyhow::Result<(TSaslClientReadTransport, TSaslClientWriteTransport)> {
        let (in_channel, out_channel) = self.channel.split()?;
        let read_transport = TFramedReadTransport::new(in_channel);
        let write_transport = TFramedWriteTransport::new(out_channel);
        Ok((read_transport, write_transport))
    }
}

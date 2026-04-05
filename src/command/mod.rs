mod commands;
mod response;

pub use commands::*;
use nusb::{
    transfer::{Bulk, In, Out},
    Interface,
};
pub use response::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{Error, FromBuffer, USB_TIMEOUT};

const COMPLETE_RESPONSE_LENGTH: u32 = 16;
const COMPLETE_RESPONSE_MAGIC: u32 = 0x0a6fe000;

#[derive(Clone)]
pub struct CommandTransaction {
    in_endpoint: u8,
    out_endpoint: u8,
    interface: Interface,
    sequence: u32,
}

impl CommandTransaction {
    pub fn new(in_endpoint: u8, out_endpoint: u8, interface: Interface) -> Self {
        Self {
            in_endpoint,
            out_endpoint,
            interface,
            sequence: 0,
        }
    }

    pub async fn execute<
        const COMMAND_ID: u32,
        const MAX_RESPONSE_LENGTH: u32,
        const MIN_RESPONSE_LENGTH: u32,
        const NPARAM: usize,
    >(
        &mut self,
        command: Command<COMMAND_ID, MAX_RESPONSE_LENGTH, MIN_RESPONSE_LENGTH, NPARAM>,
    ) -> Result<Vec<u8>, Error> {
        let sequence = self.send(&command).await?;
        let mut result = Vec::new();

        if MAX_RESPONSE_LENGTH > 0 {
            result = self
                .receive::<MAX_RESPONSE_LENGTH, MIN_RESPONSE_LENGTH>()
                .await?;

            self.check_complete_response(&result, sequence)
                .map_err(|_| Error::PrematureComplete)?;
        }

        let complete_result = self
            .receive::<COMPLETE_RESPONSE_LENGTH, COMPLETE_RESPONSE_LENGTH>()
            .await?;

        self.check_complete_response(&complete_result, sequence)?;

        Ok(result)
    }

    async fn send<
        const COMMAND_ID: u32,
        const MAX_RESPONSE_LENGTH: u32,
        const MIN_RESPONSE_LENGTH: u32,
        const NPARAM: usize,
    >(
        &mut self,
        command: &Command<COMMAND_ID, MAX_RESPONSE_LENGTH, MIN_RESPONSE_LENGTH, NPARAM>,
    ) -> Result<u32, Error> {
        let sequence = if command.has_sequence() {
            self.sequence += 1;
            self.sequence
        } else {
            0
        };

        let mut writer = self
            .interface
            .endpoint::<Bulk, Out>(self.out_endpoint)?
            .writer(command.size())
            .with_write_timeout(USB_TIMEOUT);

        writer.write_all(&command.as_bytes(sequence)).await?;
        writer.flush_end_async().await?;

        Ok(sequence)
    }

    async fn receive<const MAX_RESPONSE_LENGTH: u32, const MIN_RESPONSE_LENGTH: u32>(
        &mut self,
    ) -> Result<Vec<u8>, Error> {
        let mut reader = self
            .interface
            .endpoint::<Bulk, In>(self.in_endpoint)?
            .reader(MAX_RESPONSE_LENGTH as usize)
            .with_read_timeout(USB_TIMEOUT);
        let mut response = vec![0; MAX_RESPONSE_LENGTH as usize];
        let length = reader.read(&mut response).await?;

        if length < MIN_RESPONSE_LENGTH as usize || length > MAX_RESPONSE_LENGTH as usize {
            Err(Error::Receive(response.len(), MIN_RESPONSE_LENGTH))
        } else {
            Ok(response)
        }
    }

    fn check_complete_response(&self, result: &[u8], sequence: u32) -> Result<(), Error> {
        if result.len() == COMPLETE_RESPONSE_LENGTH as usize {
            if u32::from_buffer(&result[0..4]) == COMPLETE_RESPONSE_MAGIC {
                let result_sequence = u32::from_buffer(&result[4..8]);

                if result_sequence != sequence {
                    return Err(Error::InvalidSequence(result_sequence, sequence));
                }
            }
        }

        Ok(())
    }
}

const MAGIC_NUMBER: u32 = 0x06022009;

pub struct Command<
    const COMMAND_ID: u32,
    const MAX_RESPONSE_LENGTH: u32,
    const MIN_RESPONSE_LENGTH: u32,
    const NPARAM: usize,
> {
    has_sequence: bool,
    parameters: [u32; NPARAM],
}

impl<
        const COMMAND_ID: u32,
        const MAX_RESPONSE_LENGTH: u32,
        const MIN_RESPONSE_LENGTH: u32,
        const NPARAM: usize,
    > Command<COMMAND_ID, MAX_RESPONSE_LENGTH, MIN_RESPONSE_LENGTH, NPARAM>
{
    pub fn has_sequence(&self) -> bool {
        self.has_sequence
    }

    pub fn as_bytes(&self, sequence: u32) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.size());

        bytes.extend(MAGIC_NUMBER.to_le_bytes());
        bytes.extend(sequence.to_le_bytes());
        bytes.extend(MAX_RESPONSE_LENGTH.to_le_bytes());
        bytes.extend(COMMAND_ID.to_le_bytes());
        bytes.extend([0u8; size_of::<u32>()]);

        for parameter in self.parameters {
            bytes.extend(parameter.to_le_bytes());
        }

        bytes
    }

    pub const fn size(&self) -> usize {
        (5 + NPARAM) * size_of::<u32>()
    }
}

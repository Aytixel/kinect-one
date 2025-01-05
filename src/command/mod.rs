mod commands;
mod response;

use std::sync::Arc;

pub use commands::*;
pub use response::*;
use rusb::{DeviceHandle, UsbContext};

use crate::{Error, FromBuffer, TIMEOUT};

const COMPLETE_RESPONSE_LENGTH: usize = 16;
const COMPLETE_RESPONSE_MAGIC: u32 = 0x0a6fe000;

#[derive(Clone)]
pub struct CommandTransaction<C: UsbContext> {
    in_endpoint: u8,
    out_endpoint: u8,
    device_handle: Arc<DeviceHandle<C>>,
    sequence: u32,
}

impl<C: UsbContext> CommandTransaction<C> {
    pub fn new(in_endpoint: u8, out_endpoint: u8, device_handle: Arc<DeviceHandle<C>>) -> Self {
        Self {
            in_endpoint,
            out_endpoint,
            device_handle,
            sequence: 0,
        }
    }

    pub fn execute<
        const COMMAND_ID: u32,
        const MAX_RESPONSE_LENGTH: u32,
        const MIN_RESPONSE_LENGTH: u32,
        const NPARAM: usize,
    >(
        &mut self,
        command: Command<COMMAND_ID, MAX_RESPONSE_LENGTH, MIN_RESPONSE_LENGTH, NPARAM>,
    ) -> Result<Vec<u8>, Error> {
        let sequence = self.send(&command)?;
        let mut result = Vec::new();

        if MAX_RESPONSE_LENGTH > 0 {
            result = self.receive(MAX_RESPONSE_LENGTH, MIN_RESPONSE_LENGTH)?;

            self.check_complete_response(&result, sequence)
                .map_err(|_| Error::PrematureComplete)?;
        }

        let complete_result = self.receive(
            COMPLETE_RESPONSE_LENGTH as u32,
            COMPLETE_RESPONSE_LENGTH as u32,
        )?;

        self.check_complete_response(&complete_result, sequence)?;

        Ok(result)
    }

    fn send<
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

        let length = match self.device_handle.write_bulk(
            self.out_endpoint,
            &command.as_bytes(sequence),
            TIMEOUT,
        ) {
            Ok(length) => length,
            Err(error) => {
                if let rusb::Error::Pipe = error {
                    self.device_handle.clear_halt(self.out_endpoint)?;
                }

                return Err(error.into());
            }
        };

        if length != command.size() {
            Err(Error::Send(length, command.size()))
        } else {
            Ok(sequence)
        }
    }

    fn receive(&self, max_length: u32, min_length: u32) -> Result<Vec<u8>, Error> {
        let mut buffer = vec![0u8; max_length as usize];
        let length = match self
            .device_handle
            .read_bulk(self.in_endpoint, &mut buffer, TIMEOUT)
        {
            Ok(length) => length,
            Err(error) => {
                if let rusb::Error::Pipe = error {
                    self.device_handle.clear_halt(self.in_endpoint)?;
                }

                return Err(error.into());
            }
        };

        if length < min_length as usize {
            Err(Error::Receive(length, min_length))
        } else {
            Ok(buffer.drain(..length).collect())
        }
    }

    fn check_complete_response(&self, result: &[u8], sequence: u32) -> Result<(), Error> {
        if result.len() == COMPLETE_RESPONSE_LENGTH {
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

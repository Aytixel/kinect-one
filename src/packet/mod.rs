use std::fmt::{self, Debug};

use crate::processor::ProcessTrait;

pub mod parser;

/// Data packet with depth information.
#[derive(Clone)]
pub struct DepthPacket {
    pub sequence: u32,
    pub timestamp: u32,
    /// Depth data.
    pub buffer: Vec<u8>,
}

impl ProcessTrait for DepthPacket {}

impl Debug for DepthPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DepthPacket")
            .field("sequence", &self.sequence)
            .field("timestamp", &self.timestamp)
            .field("buffer_length", &self.buffer.len())
            .finish()
    }
}

/// Packet with JPEG data.
#[derive(Clone)]
pub struct ColorPacket {
    pub sequence: u32,
    pub timestamp: u32,
    pub exposure: f32,
    pub gain: f32,
    pub gamma: f32,
    /// JPEG data.
    pub jpeg_buffer: Vec<u8>,
}

impl ProcessTrait for ColorPacket {}

impl Debug for ColorPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ColorPacket")
            .field("sequence", &self.sequence)
            .field("timestamp", &self.timestamp)
            .field("exposure", &self.exposure)
            .field("gain", &self.gain)
            .field("gamma", &self.gamma)
            .field("jpeg_buffer_length", &self.jpeg_buffer.len())
            .finish()
    }
}

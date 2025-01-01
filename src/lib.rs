mod command;
mod device;
mod packet;
mod settings;

pub mod data;
pub mod processor;

use std::io;

use thiserror::Error;

pub use device::{Device, DeviceInfo};

pub mod config {
    pub use crate::settings::{ColorSettingCommandType, LedId, LedMode, LedSettings};

    pub const DEPTH_FRAME_SIZE: usize = 512 * 424;

    /// Configuration of depth processing.
    #[derive(Debug, Clone, Copy)]
    pub struct Config {
        // Clip at this minimum distance (meter)
        pub min_depth: f32,
        // Clip at this maximum distance (meter)
        pub max_depth: f32,

        // Remove some "flying pixels"
        pub enable_bilateral_filter: bool,
        // Remove pixels on edges because ToF cameras produce noisy edges
        pub enable_edge_aware_filter: bool,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                min_depth: 0.5,
                max_depth: 4.5,
                enable_bilateral_filter: true,
                enable_edge_aware_filter: true,
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Transfer(#[from] nusb::transfer::TransferError),
    #[error(transparent)]
    ActiveConfiguration(#[from] nusb::descriptors::ActiveConfigurationError),
    #[error("Processing error: {0}")]
    Processing(Box<dyn std::error::Error>),
    #[error("No Kinect connected")]
    NoDevice,
    #[error("The number of sent bytes differs, expected {1} got {0}")]
    Send(usize, usize),
    #[error("Not enough byte received, expected {1} got {0}")]
    Receive(usize, u32),
    #[error("Responded with the wrong sequence number, expected {1} got {0}")]
    InvalidSequence(u32, u32),
    #[error("Received a premature complete response")]
    PrematureComplete,
    #[error("Max iso packet for endpoint {0:x} is too small, expected {2}, got {1}")]
    MaxIsoPacket(u8, u16, u16),
    #[error("Serial number reported {1} differs from serial number {0} in device protocol")]
    SerialNumber(String, String),
}

trait FromBuffer {
    fn from_buffer(bytes: &[u8]) -> Self;
}

impl FromBuffer for f32 {
    fn from_buffer(bytes: &[u8]) -> Self {
        let mut buffer = [0u8; 4];

        buffer.copy_from_slice(&bytes[..4]);
        f32::from_le_bytes(buffer)
    }
}

impl FromBuffer for u32 {
    fn from_buffer(bytes: &[u8]) -> Self {
        let mut buffer = [0u8; 4];

        buffer.copy_from_slice(&bytes[..4]);
        u32::from_le_bytes(buffer)
    }
}

impl FromBuffer for u16 {
    fn from_buffer(bytes: &[u8]) -> Self {
        let mut buffer = [0u8; 2];

        buffer.copy_from_slice(&bytes[..2]);
        u16::from_le_bytes(buffer)
    }
}

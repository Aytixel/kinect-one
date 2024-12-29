mod camera;
mod command;
mod device;
mod settings;

use std::io;

use thiserror::Error;

pub use device::{Device, DeviceInfo};

pub mod config {
    pub use crate::camera::{ColorParams, IrParams};
    pub use crate::settings::{LedId, LedMode, LedSettings};

    /// Configuration of depth processing.
    #[derive(Clone)]
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
}

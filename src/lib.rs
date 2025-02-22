mod command;
mod device;
mod packet;
mod settings;

pub mod data;
pub mod processor;

use std::{any::type_name, collections::VecDeque, io, ptr::read_unaligned, time::Duration};

use packet::{ColorPacket, DepthPacket};
use thiserror::Error;

pub use device::{Device, DeviceEnumerator, DeviceInfo};

const TIMEOUT: Duration = Duration::from_millis(1000);

pub const DEPTH_WIDTH: usize = 512;
pub const DEPTH_HEIGHT: usize = 424;
pub const DEPTH_SIZE: usize = DEPTH_WIDTH * DEPTH_HEIGHT;

pub const COLOR_WIDTH: usize = 1920;
pub const COLOR_HEIGHT: usize = 1080;
pub const COLOR_SIZE: usize = COLOR_WIDTH * COLOR_HEIGHT;

pub const LUT_SIZE: usize = 2048;

pub mod config {
    pub use crate::settings::{ColorSettingCommandType, LedId, LedMode, LedSettings};

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
    Usb(#[from] rusb::Error),
    #[error(transparent)]
    UsbTransfer(#[from] rusb_async::Error),
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
    #[error("Insufficient size can't read {0}")]
    UnalignedRead(&'static str),
    #[error("{0} can happen only while running")]
    OnlyWhileRunning(&'static str),
    #[error("Can't set ir state, device handle is borrowed multiple times")]
    IrState,
}

trait ReadUnaligned: Sized {
    fn read_unaligned(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() >= Self::size() {
            Ok(unsafe { read_unaligned(bytes.as_ptr() as *const Self) })
        } else {
            Err(Error::UnalignedRead(type_name::<Self>()))
        }
    }

    fn size() -> usize {
        size_of::<Self>()
    }
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

pub struct PacketSync {
    color_packet: Option<ColorPacket>,
    depth_packet: VecDeque<DepthPacket>,
}

impl PacketSync {
    pub fn new() -> Self {
        Self {
            color_packet: None,
            depth_packet: VecDeque::with_capacity(10),
        }
    }

    pub fn push_color_packet(&mut self, color_packet: ColorPacket) {
        self.color_packet = Some(color_packet);
    }

    pub fn push_depth_packet(&mut self, depth_packet: DepthPacket) {
        self.depth_packet.push_back(depth_packet);
    }

    pub fn poll_packets(&mut self) -> Option<(ColorPacket, DepthPacket)> {
        if let Some(color_packet) = self.color_packet.take() {
            if let Some(depth_packet_position) = self
                .depth_packet
                .iter()
                .position(|depth_packet| depth_packet.timestamp > color_packet.timestamp)
            {
                self.depth_packet.drain(..depth_packet_position);

                return Some((color_packet, self.depth_packet.pop_front().unwrap()));
            }

            self.color_packet = Some(color_packet);
        }

        return None;
    }

    pub fn clear(&mut self) {
        self.color_packet = None;
        self.depth_packet.clear();
    }
}

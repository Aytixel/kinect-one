#[cfg(feature = "moz_rgb")]
mod moz;
#[cfg(feature = "turbo_rgb")]
mod turbo;
#[cfg(feature = "zune_rgb")]
mod zune;

use std::fmt::{self, Debug};

#[cfg(feature = "moz_rgb")]
pub use moz::*;
#[cfg(feature = "turbo_rgb")]
pub use turbo::*;
#[cfg(feature = "zune_rgb")]
pub use zune::*;

pub use crate::packet::ColorPacket;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    RGB,
    RGBA,
    YCbCr,
    BGR,
    BGRA,
    Unknown,
}

impl ColorSpace {
    pub const fn bytes_per_pixel(&self) -> usize {
        match self {
            ColorSpace::YCbCr | ColorSpace::RGB | ColorSpace::BGR => 3,
            ColorSpace::BGRA | ColorSpace::RGBA => 4,
            ColorSpace::Unknown => 0,
        }
    }

    pub const fn has_alpha(&self) -> bool {
        matches!(self, Self::RGBA | Self::BGRA)
    }

    pub const fn alpha_position(&self) -> Option<usize> {
        match self {
            ColorSpace::RGBA => Some(3),
            ColorSpace::BGRA => Some(3),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct ColorFrame {
    pub color_space: ColorSpace,
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<u8>,

    pub sequence: u32,
    pub timestamp: u32,
    pub exposure: f32,
    pub gain: f32,
    pub gamma: f32,
}

impl Debug for ColorFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("color_space", &self.color_space)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("buffer_length", &self.buffer.len())
            .field("sequence", &self.sequence)
            .field("timestamp", &self.timestamp)
            .field("exposure", &self.exposure)
            .field("gain", &self.gain)
            .field("gamma", &self.gamma)
            .finish()
    }
}

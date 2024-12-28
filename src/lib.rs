mod camera;
mod device;

use thiserror::Error;

pub use device::{Device, DeviceInfo};

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Usb(#[from] nusb::Error),
    #[error("No Kinect connected")]
    NoDevice,
}

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

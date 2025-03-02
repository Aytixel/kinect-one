mod closed;
mod opened;

use std::fmt::Debug;

pub use closed::Closed;
use nusb::list_devices;
pub use opened::Opened;

use crate::Error;

pub const VENDOR_ID: u16 = 0x045E;
pub const PRODUCT_ID: u16 = 0x02D8;
pub const PRODUCT_ID_PREVIEW: u16 = 0x02C4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceId {
    pub bus: u8,
    pub address: u8,
}

pub trait DeviceInfo: Debug {
    /// Get device id.
    fn id(&self) -> DeviceId;
}

/// Find, open, and control Kinect v2 devices.
#[derive(Clone)]
pub struct Device<T> {
    inner: T,
}

pub struct DeviceEnumerator;

impl DeviceEnumerator {
    /// Enumerate all Kinect v2 devices.
    pub async fn enumerate() -> Result<impl Iterator<Item = Device<Closed>>, Error> {
        Ok(list_devices()
            .await?
            .filter_map(|device_info: nusb::DeviceInfo| {
                (device_info.vendor_id() == VENDOR_ID
                    && (device_info.product_id() == PRODUCT_ID
                        || device_info.product_id() == PRODUCT_ID_PREVIEW))
                    .then_some(Device {
                        inner: Closed { device_info },
                    })
            }))
    }

    /// Open the first device.
    pub async fn open_default(reset: bool) -> Result<Device<Opened>, Error> {
        Self::enumerate()
            .await?
            .next()
            .ok_or(Error::NoDevice)?
            .open(reset)
            .await
    }

    pub fn from(device_info: nusb::DeviceInfo) -> Device<Closed> {
        device_info.into()
    }
}

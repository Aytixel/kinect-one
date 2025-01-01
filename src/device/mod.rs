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

pub trait DeviceInfo: Debug {
    /// Get device id.
    fn id(&self) -> nusb::DeviceId;

    /// Get device serial number.
    fn serial_number(&self) -> Option<&str>;
}

/// Find, open, and control Kinect v2 devices.
#[derive(Clone)]
pub struct Device<T> {
    inner: T,
}

impl Device<()> {
    /// Enumerate all Kinect v2 devices.
    pub fn enumerate_device() -> Result<impl Iterator<Item = Device<Closed>>, Error> {
        Ok(list_devices()?.filter_map(|device_info| {
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
        Device::enumerate_device()?
            .next()
            .ok_or(Error::NoDevice)?
            .open(reset)
            .await
    }

    pub fn from(device_info: nusb::DeviceInfo) -> Device<Closed> {
        device_info.into()
    }
}

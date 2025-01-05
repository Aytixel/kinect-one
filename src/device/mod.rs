mod closed;
mod opened;

use std::fmt::Debug;

pub use closed::Closed;
pub use opened::Opened;
use rusb::{devices, DeviceList, GlobalContext, UsbContext};

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

pub struct DeviceEnumerator {
    devices: DeviceList<GlobalContext>,
}

impl DeviceEnumerator {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            devices: devices()?,
        })
    }

    /// Enumerate all Kinect v2 devices.
    pub fn enumerate_device<'a>(
        &'a mut self,
    ) -> Result<impl Iterator<Item = Device<Closed<GlobalContext>>> + 'a, Error> {
        self.devices = devices()?;

        Ok(self
            .devices
            .iter()
            .filter_map(|device: rusb::Device<GlobalContext>| {
                let device_descriptor = device.device_descriptor().ok()?;

                (device_descriptor.vendor_id() == VENDOR_ID
                    && (device_descriptor.product_id() == PRODUCT_ID
                        || device_descriptor.product_id() == PRODUCT_ID_PREVIEW))
                    .then_some(Device {
                        inner: Closed { device },
                    })
            }))
    }

    /// Open the first device.
    pub fn open_default(&mut self, reset: bool) -> Result<Device<Opened<GlobalContext>>, Error> {
        self.enumerate_device()?
            .next()
            .ok_or(Error::NoDevice)?
            .open(reset)
    }

    pub fn from<C: UsbContext>(device: rusb::Device<C>) -> Device<Closed<C>> {
        device.into()
    }
}

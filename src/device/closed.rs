use std::fmt::{self, Debug};

use rusb::UsbContext;

use crate::Error;

use super::{Device, DeviceId, DeviceInfo, Opened};

#[derive(Clone)]
pub struct Closed<C: UsbContext> {
    pub device: rusb::Device<C>,
}

impl<C: UsbContext> Device<Closed<C>> {
    /// Open the device.
    pub fn open(self, reset: bool) -> Result<Device<Opened<C>>, Error> {
        if reset {
            self.inner.device.open()?.reset()?;
        }

        Ok(Device {
            inner: Opened::new(self.inner.device)?,
        })
    }
}

impl<C: UsbContext> DeviceInfo for Device<Closed<C>> {
    fn id(&self) -> DeviceId {
        DeviceId {
            bus: self.inner.device.bus_number(),
            address: self.inner.device.address(),
        }
    }
}

impl<C: UsbContext> Debug for Device<Closed<C>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.device.fmt(f)
    }
}

impl<C: UsbContext> From<rusb::Device<C>> for Device<Closed<C>> {
    fn from(device: rusb::Device<C>) -> Self {
        Device {
            inner: Closed { device },
        }
    }
}

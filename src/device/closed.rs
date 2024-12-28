use std::fmt::{self, Debug};

use crate::Error;

use super::{Device, DeviceInfo, Opened};

#[derive(Clone)]
pub struct Closed {
    pub(super) device_info: nusb::DeviceInfo,
}

impl Device<Closed> {
    pub fn open(self) -> Result<Device<Opened>, Error> {
        Ok(Device {
            inner: Opened::new(self.inner.device_info.open()?, self.inner.device_info),
        })
    }
}

impl DeviceInfo for Device<Closed> {
    fn id(&self) -> nusb::DeviceId {
        self.inner.device_info.id()
    }

    fn serial_number(&self) -> Option<&str> {
        self.inner.device_info.serial_number()
    }
}

impl Debug for Device<Closed> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.device_info.fmt(f)
    }
}

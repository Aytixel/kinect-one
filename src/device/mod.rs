mod closed;
mod opened;

use std::fmt::Debug;

use nusb::list_devices;

pub use closed::Closed;
pub use opened::Opened;

use crate::Error;

pub const VENDOR_ID: u16 = 0x045E;
pub const PRODUCT_ID: u16 = 0x02D8;
pub const PRODUCT_ID_PREVIEW: u16 = 0x02C4;

pub trait DeviceInfo: Debug {
    fn id(&self) -> nusb::DeviceId;

    fn serial_number(&self) -> Option<&str>;
}

#[derive(Clone)]
pub struct Device<T> {
    inner: T,
}

impl Device<()> {
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

    pub async fn open_default() -> Result<Device<Opened>, Error> {
        Device::enumerate_device()?
            .next()
            .ok_or(Error::NoDevice)?
            .open()
            .await
    }
}

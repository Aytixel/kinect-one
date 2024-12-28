use std::fmt::{self, Debug};

use crate::{
    camera::{ColorParams, IrParams},
    Config,
};

use super::{Closed, Device, DeviceInfo};

#[derive(Clone)]
pub struct Opened {
    pub(super) device: nusb::Device,
    pub(super) device_info: nusb::DeviceInfo,
    color_params: ColorParams,
    ir_params: IrParams,
}

impl Opened {
    pub(super) fn new(device: nusb::Device, device_info: nusb::DeviceInfo) -> Self {
        Self {
            device,
            device_info,
            color_params: Default::default(),
            ir_params: Default::default(),
        }
    }
}

impl Device<Opened> {
    pub fn get_color_params(&self) -> &ColorParams {
        &self.inner.color_params
    }

    pub fn get_ir_params(&self) -> &IrParams {
        &self.inner.ir_params
    }

    pub fn set_color_params(&mut self, color_params: &ColorParams) {
        self.inner.color_params = color_params.clone();
    }

    pub fn set_ir_params(&mut self, ir_params: &IrParams) {
        self.inner.ir_params = ir_params.clone();

        todo!("set ir params in the depth packet processor");
    }

    pub fn set_config(&mut self) {
        todo!("set config in the depth packet processor");
    }

    pub fn close(self) -> Device<Closed> {
        Device {
            inner: Closed {
                device_info: self.inner.device_info,
            },
        }
    }
}

impl DeviceInfo for Device<Opened> {
    fn id(&self) -> nusb::DeviceId {
        self.inner.device_info.id()
    }

    fn serial_number(&self) -> Option<&str> {
        self.inner.device_info.serial_number()
    }
}

impl Debug for Device<Opened> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.device_info.fmt(f)
    }
}

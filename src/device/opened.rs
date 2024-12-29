use std::fmt::{self, Debug};

use nusb::transfer::{ControlOut, ControlType, EndpointType, Recipient};

use crate::{
    camera::{ColorParams, IrParams},
    command::{led_setting_command, CommandTransaction},
    settings::LedSettings,
    Error,
};

use super::{Closed, Device, DeviceInfo};

#[derive(Clone, Copy)]
#[repr(u8)]
enum InterfaceId {
    ControlAndRgb = 0,
    Ir = 1,
}

#[derive(Clone, Copy)]
#[repr(u16)]
enum Feature {
    U1Enable = 48,
    U2Enable = 49,
    FunctionSuspend = 0,
}

impl Feature {
    fn recipient(&self) -> Recipient {
        match self {
            Feature::U1Enable | Feature::U2Enable => Recipient::Device,
            Feature::FunctionSuspend => Recipient::Interface,
        }
    }
}

const CONTROL_IN_ENDPOINT: u8 = 0x81;
const CONTROL_OUT_ENDPOINT: u8 = 0x02;
const RGB_ENDPOINT: u8 = 0x83;
const IR_ENDPOINT: u8 = 0x84;

const SET_ISOCH_DELAY: u8 = 0x31;
const REQUEST_SET_SEL: u8 = 0x30;
const REQUEST_SET_FEATURE: u8 = 0x03;
const DT_SS_ENDPOINT_COMPANION: u8 = 0x30;

#[derive(Clone)]
pub struct Opened {
    command_transaction: CommandTransaction,
    interfaces: [nusb::Interface; 2],
    device: nusb::Device,
    device_info: nusb::DeviceInfo,
    color_params: ColorParams,
    ir_params: IrParams,
}

impl Opened {
    pub(super) async fn new(device_info: nusb::DeviceInfo) -> Result<Self, Error> {
        let device = device_info.open()?;

        if device.active_configuration()?.configuration_value() != 1 {
            device.set_configuration(1)?;
        }

        let interfaces = [
            device.claim_interface(InterfaceId::ControlAndRgb as u8)?,
            device.claim_interface(InterfaceId::Ir as u8)?,
        ];

        // set isochronous delay
        device
            .control_out(ControlOut {
                control_type: ControlType::Standard,
                recipient: Recipient::Device,
                request: SET_ISOCH_DELAY,
                value: 40,
                index: 0,
                data: &[],
            })
            .await
            .status?;

        let opened_device = Self {
            command_transaction: CommandTransaction::new(
                CONTROL_IN_ENDPOINT,
                CONTROL_OUT_ENDPOINT,
                interfaces[InterfaceId::ControlAndRgb as u8 as usize].clone(),
            ),
            interfaces,
            device,
            device_info,
            color_params: Default::default(),
            ir_params: Default::default(),
        };

        // set power state latencies
        opened_device.set_sel(&[0x55, 0, 0x55, 0, 0, 0]).await?;
        opened_device.set_ir_state(false)?;
        // enable power states
        opened_device.set_feature(Feature::U1Enable).await?;
        opened_device.set_feature(Feature::U2Enable).await?;
        opened_device
            .set_video_transfer_function_state(false)
            .await?;
        // get ir max packet size
        if let Some(max_iso_packet_size) = opened_device.get_max_iso_packet_size(1, 1, IR_ENDPOINT)
        {
            if max_iso_packet_size < 0x8400 {
                return Err(Error::MaxIsoPacket(
                    IR_ENDPOINT,
                    max_iso_packet_size,
                    0x8400,
                ));
            }
        }

        Ok(opened_device)
    }

    async fn set_sel(&self, data: &[u8]) -> Result<(), Error> {
        Ok(self
            .device
            .control_out(ControlOut {
                control_type: ControlType::Standard,
                recipient: Recipient::Device,
                request: REQUEST_SET_SEL,
                value: 0,
                index: 0,
                data,
            })
            .await
            .status?)
    }

    async fn set_feature(&self, feature: Feature) -> Result<(), Error> {
        Ok(self
            .device
            .control_out(ControlOut {
                control_type: ControlType::Standard,
                recipient: feature.recipient(),
                request: REQUEST_SET_FEATURE,
                value: feature as u16,
                index: 0,
                data: &[],
            })
            .await
            .status?)
    }

    async fn set_feature_function_suspend(
        &self,
        low_power_suspend: bool,
        function_remote_wake: bool,
    ) -> Result<(), Error> {
        let feature = Feature::FunctionSuspend;
        let suspend_options = (low_power_suspend as u16) + ((function_remote_wake as u16) << 1);

        Ok(self
            .device
            .control_out(ControlOut {
                control_type: ControlType::Standard,
                recipient: feature.recipient(),
                request: REQUEST_SET_FEATURE,
                value: feature as u16,
                index: suspend_options << 8 | 0,
                data: &[],
            })
            .await
            .status?)
    }

    fn get_max_iso_packet_size(
        &self,
        configuration_value: u8,
        alternate_setting_index: usize,
        endpoint_address: u8,
    ) -> Option<u16> {
        let Some(configuration) = self
            .device
            .configurations()
            .find(|configuration| configuration.configuration_value() == configuration_value)
        else {
            return None;
        };

        for interface in configuration.interfaces() {
            let Some(interface_alt_setting) = interface.alt_settings().nth(alternate_setting_index)
            else {
                continue;
            };
            let Some(endpoint) = interface_alt_setting.endpoints().find(|endpoint| {
                endpoint.address() == endpoint_address
                    && endpoint.transfer_type() == EndpointType::Isochronous
            }) else {
                continue;
            };

            return endpoint.descriptors().find_map(|descriptor| {
                (descriptor.descriptor_type() == DT_SS_ENDPOINT_COMPANION)
                    .then_some(u16::from_le_bytes([descriptor[4], descriptor[5]]))
            });
        }

        None
    }

    fn get_interface(&self, interface_id: InterfaceId) -> &nusb::Interface {
        &self.interfaces[interface_id as u8 as usize]
    }

    fn set_ir_state(&self, enabled: bool) -> Result<(), Error> {
        Ok(self
            .get_interface(InterfaceId::Ir)
            .set_alt_setting(!enabled as u8)?)
    }

    async fn set_video_transfer_function_state(&self, enabled: bool) -> Result<(), Error> {
        self.set_feature_function_suspend(!enabled, !enabled).await
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

    pub async fn set_led_status(&mut self, led_settings: LedSettings) -> Result<(), Error> {
        self.inner
            .command_transaction
            .execute(&led_setting_command(led_settings))
            .await?;

        Ok(())
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

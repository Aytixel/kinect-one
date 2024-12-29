use std::{
    fmt::{self, Debug},
    time::Duration,
};

use nusb::transfer::{ControlOut, ControlType, EndpointType, Recipient};

use crate::{
    command::{
        color_setting_command, init_streams_command, led_setting_command,
        read_depth_params_command, read_firware_versions_command, read_p0_tables_command,
        read_rgb_params_command, read_serial_number_command, set_mode_command,
        set_stream_state_command, stop_command, CommandTransaction,
    },
    data::{ColorParams, FirwareVersion, IrParams},
    settings::{ColorSettingCommandType, LedSettings},
    Error, FromBuffer,
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
const RGB_IN_ENDPOINT: u8 = 0x83;
const IR_IN_ENDPOINT: u8 = 0x84;

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
        if let Some(max_iso_packet_size) =
            opened_device.get_max_iso_packet_size(1, 1, IR_IN_ENDPOINT)
        {
            if max_iso_packet_size < 0x8400 {
                return Err(Error::MaxIsoPacket(
                    IR_IN_ENDPOINT,
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
                    .then_some(u16::from_buffer(&descriptor[4..6]))
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
    /// Start data processing with both RGB and depth streams.
    /// All above configuration must only be called before start() or after stop().
    ///
    /// FrameListener will receive frames when the device is running.
    pub async fn start(&mut self) -> Result<(), Error> {
        self.start_streams(true, true).await
    }

    /// Start data processing with or without some streams.
    /// FrameListener will receive enabled frames when the device is running.
    ///
    /// # Arguments
    ///
    /// * `enable_rgb` - Whether to enable rgb stream.
    /// * `enable_depth` - Whether to enable depth stream.
    pub async fn start_streams(
        &mut self,
        enable_rgb: bool,
        enable_depth: bool,
    ) -> Result<(), Error> {
        self.inner.set_video_transfer_function_state(true).await?;

        let usb_serial_number = self.serial_number().unwrap_or_default().to_string();
        let device_protocol_serial_number = self.get_serial_number().await?;

        if device_protocol_serial_number != usb_serial_number {
            return Err(Error::SerialNumber(
                device_protocol_serial_number,
                usb_serial_number,
            ));
        }

        let ir_params = IrParams::from(
            self.inner
                .command_transaction
                .execute(read_depth_params_command())
                .await?
                .as_slice(),
        );

        self.set_ir_params(&ir_params);

        let p0_tables = self
            .inner
            .command_transaction
            .execute(read_p0_tables_command())
            .await?;
        let color_params = ColorParams::from(
            self.inner
                .command_transaction
                .execute(read_rgb_params_command())
                .await?
                .as_slice(),
        );

        self.set_color_params(&color_params);

        todo!("deserialize data and set value");

        self.inner
            .command_transaction
            .execute(set_mode_command(true, 0x00640064))
            .await?;
        self.inner
            .command_transaction
            .execute(set_mode_command(false, 0))
            .await?;
        self.inner
            .command_transaction
            .execute(init_streams_command())
            .await?;
        self.inner.set_ir_state(true)?;
        self.inner
            .command_transaction
            .execute(set_stream_state_command(true))
            .await?;

        todo!();
        if enable_depth {}

        if enable_rgb {}

        Ok(())
    }

    pub async fn get_firware_versions(&mut self) -> Result<Vec<FirwareVersion>, Error> {
        let buffer = self
            .inner
            .command_transaction
            .execute(read_firware_versions_command())
            .await?;
        const FIRWARE_VERSION_SIZE: usize = 16;

        Ok((0..(buffer.len() / FIRWARE_VERSION_SIZE))
            .map(|index| FirwareVersion::from(&buffer[index * FIRWARE_VERSION_SIZE..]))
            .collect())
    }

    pub async fn get_serial_number(&mut self) -> Result<String, Error> {
        let mut buffer = self
            .inner
            .command_transaction
            .execute(read_serial_number_command())
            .await?;

        buffer.retain(|char| *char != 0);

        Ok(String::from_utf8_lossy(&buffer).to_string())
    }

    /// Get current color parameters.
    pub fn get_color_params(&self) -> &ColorParams {
        &self.inner.color_params
    }

    /// Get current depth parameters.
    pub fn get_ir_params(&self) -> &IrParams {
        &self.inner.ir_params
    }

    /// Replace factory preset color camera parameters.
    /// We do not have a clear understanding of the meaning of the parameters right now.
    /// You probably want to leave it as it is.
    pub fn set_color_params(&mut self, color_params: &ColorParams) {
        self.inner.color_params = color_params.clone();
    }

    /// Replace factory preset depth camera parameters.
    /// This decides accuracy in depth images. You are recommended to provide calibrated values.
    pub fn set_ir_params(&mut self, ir_params: &IrParams) {
        self.inner.ir_params = ir_params.clone();

        todo!("set ir params in the depth packet processor");
    }

    /// Configure depth processing.
    pub fn set_config(&mut self) {
        todo!("set config in the depth packet processor");
    }

    /// Sets the RGB camera to fully automatic exposure setting.
    /// Exposure compensation: negative value gives an underexposed image, positive gives an overexposed image.
    ///
    /// # Arguments
    ///
    /// * `exposure_compensation` - Exposure compensation, range [-2.0, 2.0]
    pub async fn set_color_auto_exposure(
        &mut self,
        exposure_compensation: f32,
    ) -> Result<(), Error> {
        self.inner
            .command_transaction
            .execute(color_setting_command(ColorSettingCommandType::SetAcs, 0))
            .await?;
        self.inner
            .command_transaction
            .execute(color_setting_command(
                ColorSettingCommandType::SetExposureMode,
                0,
            ))
            .await?;
        self.inner
            .command_transaction
            .execute(color_setting_command(
                ColorSettingCommandType::SetExposureCompensation,
                exposure_compensation.clamp(-2.0, 2.0).to_bits(),
            ))
            .await?;

        Ok(())
    }

    /// Sets a flicker-free exposure time of the RGB camera in pseudo-ms, value in range [0.0, 640] ms.
    /// The actual frame integration time is set to a multiple of fluorescent light period
    /// that is shorter than the requested time e.g. requesting 16 ms will set 10 ms
    /// in Australia (100Hz light flicker), 8.33 ms in USA (120Hz light flicker).
    /// The gain is automatically set to compensate for the reduced integration time,
    /// as if the gain was set to 1.0 and the integration time was the requested value.
    ///
    /// Requesting less than a single fluorescent light period will set the integration time
    /// to the requested value, so the image brightness will flicker.
    ///
    /// To set the shortest non-flickering integration period for any country, simply set
    /// a pseudo-exposure time of between (10.0, 16.667) ms, which will automatically drop
    /// the integration time to 10 or 8.3 ms depending on country, while setting the analog
    /// gain control to a brighter value.
    ///
    /// # Arguments
    ///
    /// * `pseudo_exposure_time` - Pseudo-exposure time in milliseconds, range (0.0, 66.0+]
    pub async fn set_color_semi_auto_exposure(
        &mut self,
        pseudo_exposure_time: Duration,
    ) -> Result<(), Error> {
        self.inner
            .command_transaction
            .execute(color_setting_command(ColorSettingCommandType::SetAcs, 0))
            .await?;
        self.inner
            .command_transaction
            .execute(color_setting_command(
                ColorSettingCommandType::SetExposureMode,
                3,
            ))
            .await?;
        self.inner
            .command_transaction
            .execute(color_setting_command(
                ColorSettingCommandType::SetExposureTimeMs,
                ((pseudo_exposure_time.as_secs_f64() / 1000.0) as f32)
                    .clamp(0.0, 66.0)
                    .to_bits(),
            ))
            .await?;

        Ok(())
    }

    /// Manually set true exposure time and analog gain of the RGB camera.
    ///
    /// # Arguments
    ///
    /// * `integration_time` - True shutter time in milliseconds, range (0.0, 66.0]
    /// * `analog_gain` - Analog gain, range [1.0, 4.0]
    pub async fn set_color_manual_exposure(
        &mut self,
        integration_time: Duration,
        analog_gain: f32,
    ) -> Result<(), Error> {
        self.inner
            .command_transaction
            .execute(color_setting_command(ColorSettingCommandType::SetAcs, 0))
            .await?;
        self.inner
            .command_transaction
            .execute(color_setting_command(
                ColorSettingCommandType::SetExposureMode,
                4,
            ))
            .await?;
        self.inner
            .command_transaction
            .execute(color_setting_command(
                ColorSettingCommandType::SetIntegrationTime,
                ((integration_time.as_secs_f64() / 1000.0) as f32)
                    .clamp(0.0, 66.0)
                    .to_bits(),
            ))
            .await?;
        self.inner
            .command_transaction
            .execute(color_setting_command(
                ColorSettingCommandType::SetAnalogGain,
                analog_gain.clamp(1.0, 4.0).to_bits(),
            ))
            .await?;

        Ok(())
    }

    /// Set an individual setting value of the RGB camera.
    pub async fn set_color_setting(
        &mut self,
        command: ColorSettingCommandType,
        value: u32,
    ) -> Result<(), Error> {
        self.inner
            .command_transaction
            .execute(color_setting_command(command, value))
            .await?;

        Ok(())
    }

    /// get an individual setting value of the RGB camera.
    pub async fn get_color_setting(
        &mut self,
        command: ColorSettingCommandType,
    ) -> Result<u32, Error> {
        let bytes = self
            .inner
            .command_transaction
            .execute(color_setting_command(command, 0))
            .await?;
        const OFFSET: usize = size_of::<u32>() * 3;

        Ok(u32::from_buffer(&bytes[OFFSET..OFFSET + 4]))
    }

    /// Set the settings of a Kinect LED.
    ///
    /// # Arguments
    ///
    /// * `led_settings` - Settings for a single LED.
    pub async fn set_led_status(&mut self, led_settings: LedSettings) -> Result<(), Error> {
        self.inner
            .command_transaction
            .execute(led_setting_command(led_settings))
            .await?;

        Ok(())
    }

    /// Stop data processing.
    pub async fn stop(&mut self) -> Result<(), Error> {
        self.inner.set_ir_state(false)?;
        self.inner
            .command_transaction
            .execute(set_mode_command(true, 0x00640064))
            .await?;
        self.inner
            .command_transaction
            .execute(set_mode_command(false, 0))
            .await?;
        self.inner
            .command_transaction
            .execute(stop_command())
            .await?;
        self.inner
            .command_transaction
            .execute(set_stream_state_command(false))
            .await?;
        self.inner
            .command_transaction
            .execute(set_mode_command(true, 0))
            .await?;
        self.inner
            .command_transaction
            .execute(set_mode_command(false, 0))
            .await?;
        self.inner.set_video_transfer_function_state(false).await
    }

    /// Shut down the device.
    pub async fn close(mut self) -> Result<Device<Closed>, Error> {
        self.stop().await?;
        self.inner
            .command_transaction
            .execute(set_mode_command(true, 0x00640064))
            .await?;
        self.inner
            .command_transaction
            .execute(set_mode_command(false, 0))
            .await?;

        #[cfg(target_os = "macos")]
        {
            /* Kinect will disappear on Mac OS X regardless during close().
             * Painstaking effort could not determine the root cause.
             * See https://github.com/OpenKinect/libfreenect2/issues/539
             *
             * Shut down Kinect explicitly on Mac and wait a fixed time.
             */
            self.inner
                .command_transaction
                .execute(crate::command::shutdown_command())
                .await?;
            tokio::time::sleep(Duration::from_secs(4 * 1000)).await;
        }

        Ok(Device {
            inner: Closed {
                device_info: self.inner.device_info,
            },
        })
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

use std::{
    fmt::{self, Debug},
    thread::sleep,
    time::Duration,
};

use nusb::{
    transfer::{
        ControlOut, ControlType, Queue, Recipient, RequestBuffer, RequestIsochronousBuffer,
        TransferType,
    },
    Interface,
};

use crate::{
    command::{
        color_setting_command, init_streams_command, led_setting_command,
        read_color_params_command, read_depth_params_command, read_firware_versions_command,
        read_p0_tables_command, read_serial_number_command, read_status_command, set_mode_command,
        set_stream_state_command, shutdown_command, stop_command, ColorSettingResponse,
        CommandTransaction,
    },
    data::{ColorParams, FirwareVersion, IrParams, P0Tables},
    packet::{
        parser::{ColorStreamParser, DepthStreamParser},
        ColorPacket, DepthPacket,
    },
    settings::{ColorSettingCommandType, LedSettings, PacketParams},
    Error, FromBuffer, ReadUnaligned,
};

use super::{Closed, Device, DeviceId, DeviceInfo};

#[derive(Clone, Copy)]
#[repr(u8)]
enum InterfaceId {
    ControlAndColor = 0,
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
const COLOR_IN_ENDPOINT: u8 = 0x83;
const IR_IN_ENDPOINT: u8 = 0x84;

const SET_ISOCH_DELAY: u8 = 0x31;
const REQUEST_SET_SEL: u8 = 0x30;
const REQUEST_SET_FEATURE: u8 = 0x03;
const DT_SS_ENDPOINT_COMPANION: u8 = 0x30;

pub struct Opened {
    command_transaction: CommandTransaction,
    device_info: nusb::DeviceInfo,
    device: nusb::Device,
    control_and_color_interface: Interface,
    ir_interface: Interface,
    color_params: ColorParams,
    ir_params: IrParams,
    p0_tables: P0Tables,
    packet_params: PacketParams,
    color_queue: Queue<RequestBuffer>,
    color_stream_parser: ColorStreamParser,
    ir_queue: Queue<RequestIsochronousBuffer>,
    depth_stream_parser: DepthStreamParser,
    running: bool,
}

impl Opened {
    pub(super) async fn new(device_info: nusb::DeviceInfo) -> Result<Self, Error> {
        let device = device_info.open().await?;

        if device.active_configuration()?.configuration_value() != 1 {
            device.set_configuration(1).await?;
        }

        let control_and_color_interface = device
            .claim_interface(InterfaceId::ControlAndColor as u8)
            .await?;
        let ir_interface = device.claim_interface(InterfaceId::Ir as u8).await?;

        // set isochronous delay
        control_and_color_interface
            .control_out(ControlOut {
                control_type: ControlType::Standard,
                recipient: Recipient::Device,
                request: SET_ISOCH_DELAY,
                value: 40,
                index: 0,
                data: &[],
            })
            .await
            .into_result()?;

        let mut opened_device = Self {
            command_transaction: CommandTransaction::new(
                CONTROL_IN_ENDPOINT,
                CONTROL_OUT_ENDPOINT,
                control_and_color_interface.clone(),
            ),
            color_params: Default::default(),
            ir_params: Default::default(),
            p0_tables: Default::default(),
            packet_params: Default::default(),
            color_queue: control_and_color_interface.bulk_in_queue(COLOR_IN_ENDPOINT),
            color_stream_parser: ColorStreamParser::new(),
            ir_queue: ir_interface.isochronous_in_queue(IR_IN_ENDPOINT),
            depth_stream_parser: DepthStreamParser::new(),
            running: false,
            control_and_color_interface,
            ir_interface,
            device_info,
            device,
        };

        // set power state latencies
        opened_device.set_sel(&[0x55, 0, 0x55, 0, 0, 0]).await?;
        opened_device.set_ir_state(false).await?;
        // enable power states
        opened_device.set_feature(Feature::U1Enable).await?;
        opened_device.set_feature(Feature::U2Enable).await?;
        opened_device
            .set_video_transfer_function_state(false)
            .await?;
        // get ir max packet size
        opened_device.packet_params.max_iso_packet_size = opened_device
            .get_max_iso_packet_size(1, 1, IR_IN_ENDPOINT)
            .await
            .unwrap_or(0);

        if opened_device.packet_params.max_iso_packet_size < 0x8400 {
            return Err(Error::MaxIsoPacket(
                IR_IN_ENDPOINT,
                opened_device.packet_params.max_iso_packet_size,
                0x8400,
            ));
        }

        Ok(opened_device)
    }

    async fn set_sel(&self, data: &[u8]) -> Result<(), Error> {
        self.control_and_color_interface
            .control_out(ControlOut {
                control_type: ControlType::Standard,
                recipient: Recipient::Device,
                request: REQUEST_SET_SEL,
                value: 0,
                index: 0,
                data,
            })
            .await
            .into_result()?;

        Ok(())
    }

    async fn set_feature(&self, feature: Feature) -> Result<(), Error> {
        self.control_and_color_interface
            .control_out(ControlOut {
                control_type: ControlType::Standard,
                recipient: feature.recipient(),
                request: REQUEST_SET_FEATURE,
                value: feature as u16,
                index: 0,
                data: &[],
            })
            .await
            .into_result()?;

        Ok(())
    }

    async fn set_feature_function_suspend(
        &self,
        low_power_suspend: bool,
        function_remote_wake: bool,
    ) -> Result<(), Error> {
        let feature = Feature::FunctionSuspend;
        let suspend_options = (low_power_suspend as u16) + ((function_remote_wake as u16) << 1);

        self.control_and_color_interface
            .control_out(ControlOut {
                control_type: ControlType::Standard,
                recipient: feature.recipient(),
                request: REQUEST_SET_FEATURE,
                value: feature as u16,
                index: suspend_options << 8 | 0,
                data: &[],
            })
            .await
            .into_result()?;

        Ok(())
    }

    async fn get_max_iso_packet_size(
        &self,
        configuration_value: u8,
        alternate_setting_index: u8,
        endpoint_address: u8,
    ) -> Option<u16> {
        let configuration = self
            .device
            .configurations()
            .find(|configuration| configuration.configuration_value() == configuration_value)?;

        for interface in configuration.interface_alt_settings() {
            if interface.alternate_setting() == alternate_setting_index {
                for endpoint in interface.endpoints() {
                    if endpoint.address() == endpoint_address
                        && endpoint.transfer_type() == TransferType::Isochronous
                    {
                        for buffer in endpoint.descriptors() {
                            if buffer.len() >= 6 && buffer[1] == DT_SS_ENDPOINT_COMPANION {
                                return Some(u16::from_buffer(&buffer[4..6]));
                            }
                        }
                    }
                }
            }
        }

        None
    }

    async fn set_ir_state(&mut self, enabled: bool) -> Result<(), Error> {
        Ok(self.ir_interface.set_alt_setting(enabled as u8).await?)
    }

    async fn set_video_transfer_function_state(&self, enabled: bool) -> Result<(), Error> {
        self.set_feature_function_suspend(!enabled, !enabled).await
    }
}

impl Device<Opened> {
    pub fn running(&self) -> bool {
        self.inner.running
    }

    /// Start data processing with both color and depth streams.
    /// All above configuration must only be called before start() or after stop().
    pub async fn start(&mut self) -> Result<(), Error> {
        if self.inner.running {
            return Ok(());
        }

        self.inner.running = true;

        self.inner.set_video_transfer_function_state(true).await?;

        let usb_serial_number = self
            .inner
            .device_info
            .serial_number()
            .unwrap_or_default()
            .to_string();
        let device_protocol_serial_number = self.get_serial_number().await?;

        if device_protocol_serial_number != usb_serial_number {
            return Err(Error::SerialNumber(
                device_protocol_serial_number,
                usb_serial_number,
            ));
        }

        self.inner.ir_params = IrParams::try_from(
            self.inner
                .command_transaction
                .execute(read_depth_params_command())
                .await?
                .as_slice(),
        )?;
        self.inner.color_params = ColorParams::try_from(
            self.inner
                .command_transaction
                .execute(read_color_params_command())
                .await?
                .as_slice(),
        )?;
        self.inner.p0_tables = P0Tables::try_from(
            self.inner
                .command_transaction
                .execute(read_p0_tables_command())
                .await?
                .as_slice(),
        )?;

        self.inner
            .command_transaction
            .execute(set_mode_command(true, 0x00640064))
            .await?;
        self.inner
            .command_transaction
            .execute(set_mode_command(false, 0))
            .await?;

        for _ in 0..50 {
            if u32::from_buffer(
                &self
                    .inner
                    .command_transaction
                    .execute(read_status_command(0x090000))
                    .await?,
            ) & 1
                != 0
            {
                break;
            }
            sleep(Duration::from_millis(100));
        }

        self.inner
            .command_transaction
            .execute(init_streams_command())
            .await?;
        self.inner.set_ir_state(true).await?;
        self.inner
            .command_transaction
            .execute(set_stream_state_command(true))
            .await?;

        Ok(())
    }

    pub async fn poll_color_packet(&mut self) -> Result<Option<ColorPacket>, Error> {
        if !self.inner.running {
            return Err(Error::OnlyWhileRunning("Reading color frame"));
        }

        for _ in 0..self.inner.packet_params.color_num_transfers {
            self.inner.color_queue.submit(RequestBuffer::new(
                self.inner.packet_params.color_transfer_size,
            ));
        }

        let mut result = None;

        while self.inner.color_queue.pending() > 0 {
            if let Some(packet) = self
                .inner
                .color_stream_parser
                .parse(self.inner.color_queue.next_complete().await.into_result()?)
            {
                result = Some(packet);
            }
        }

        Ok(result)
    }

    pub async fn poll_depth_packet(&mut self) -> Result<Option<DepthPacket>, Error> {
        if !self.inner.running {
            return Err(Error::OnlyWhileRunning("Reading depth frame"));
        }

        for _ in 0..self.inner.packet_params.ir_num_transfers {
            self.inner.ir_queue.submit(RequestIsochronousBuffer::new(
                self.inner.packet_params.max_iso_packet_size as usize,
                self.inner.packet_params.ir_packets_per_transfer as usize,
            ));
        }

        let mut result = None;

        while self.inner.ir_queue.pending() > 0 {
            for iso_packet in self.inner.ir_queue.next_complete().await.into_result()? {
                if let Some(packet) = self.inner.depth_stream_parser.parse(iso_packet) {
                    result = Some(packet);
                }
            }
        }

        Ok(result)
    }

    pub async fn get_firware_versions(&mut self) -> Result<Vec<FirwareVersion>, Error> {
        let buffer = self
            .inner
            .command_transaction
            .execute(read_firware_versions_command())
            .await?;
        const FIRWARE_VERSION_SIZE: usize = 16;
        let mut versions = Vec::new();

        for index in 0..buffer.len() / FIRWARE_VERSION_SIZE {
            versions.push(FirwareVersion::try_from(
                &buffer[index * FIRWARE_VERSION_SIZE..],
            )?);
        }

        Ok(versions)
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

    /// Get color parameters.
    pub fn get_color_params(&self) -> &ColorParams {
        &self.inner.color_params
    }

    /// Get depth parameters.
    pub fn get_ir_params(&self) -> &IrParams {
        &self.inner.ir_params
    }

    /// Get p0 tables.
    pub fn get_p0_tables(&self) -> &P0Tables {
        &self.inner.p0_tables
    }

    /// Sets the color camera to fully automatic exposure setting.
    /// Exposure compensation: negative value gives an underexposed image, positive gives an overexposed image.
    ///
    /// # Arguments
    ///
    /// * `exposure_compensation` - Exposure compensation, range [-2.0, 2.0]
    pub async fn set_color_auto_exposure(
        &mut self,
        exposure_compensation: f32,
    ) -> Result<(), Error> {
        if !self.inner.running {
            return Err(Error::OnlyWhileRunning("Setting auto exposure"));
        }

        self.set_color_setting(ColorSettingCommandType::SetAcs, 0)
            .await?;
        self.set_color_setting(ColorSettingCommandType::SetExposureMode, 0)
            .await?;
        self.set_color_setting(
            ColorSettingCommandType::SetExposureCompensation,
            exposure_compensation.clamp(-2.0, 2.0).to_bits(),
        )
        .await?;

        Ok(())
    }

    /// Sets a flicker-free exposure time of the color camera in pseudo-ms, value in range [0.0, 640] ms.
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
        if !self.inner.running {
            return Err(Error::OnlyWhileRunning("Setting semi-auto exposure"));
        }

        self.set_color_setting(ColorSettingCommandType::SetAcs, 0)
            .await?;
        self.set_color_setting(ColorSettingCommandType::SetExposureMode, 3)
            .await?;
        self.set_color_setting(
            ColorSettingCommandType::SetExposureTimeMs,
            ((pseudo_exposure_time.as_secs_f64() / 1000.0) as f32)
                .clamp(0.0, 66.0)
                .to_bits(),
        )
        .await?;

        Ok(())
    }

    /// Manually set true exposure time and analog gain of the color camera.
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
        if !self.inner.running {
            return Err(Error::OnlyWhileRunning("Setting manual exposure"));
        }

        self.set_color_setting(ColorSettingCommandType::SetAcs, 0)
            .await?;
        self.set_color_setting(ColorSettingCommandType::SetExposureMode, 4)
            .await?;
        self.set_color_setting(
            ColorSettingCommandType::SetIntegrationTime,
            ((integration_time.as_secs_f64() / 1000.0) as f32)
                .clamp(0.0, 66.0)
                .to_bits(),
        )
        .await?;
        self.set_color_setting(
            ColorSettingCommandType::SetAnalogGain,
            analog_gain.clamp(1.0, 4.0).to_bits(),
        )
        .await?;

        Ok(())
    }

    /// Set an individual setting value of the color camera.
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

    /// get an individual setting value of the color camera.
    pub async fn get_color_setting(
        &mut self,
        command: ColorSettingCommandType,
    ) -> Result<u32, Error> {
        let bytes = self
            .inner
            .command_transaction
            .execute(color_setting_command(command, 0))
            .await?;

        Ok(ColorSettingResponse::read_unaligned(&bytes)?.data)
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
        if !self.inner.running {
            return Ok(());
        }

        self.inner.running = false;

        self.inner.set_ir_state(false).await?;
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
        self.inner
            .command_transaction
            .execute(shutdown_command())
            .await?;

        Ok(Device {
            inner: Closed {
                device_info: self.inner.device_info,
            },
        })
    }
}

impl DeviceInfo for Device<Opened> {
    fn id(&self) -> DeviceId {
        DeviceId {
            bus: self.inner.device_info.busnum(),
            address: self.inner.device_info.device_address(),
        }
    }
}

impl Debug for Device<Opened> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.device_info.fmt(f)
    }
}

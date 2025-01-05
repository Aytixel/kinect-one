use std::{
    fmt::{self, Debug},
    sync::Arc,
    thread::sleep,
    time::Duration,
};

use rusb::{request_type, Direction, Recipient, RequestType, TransferType, UsbContext};
use rusb_async::TransferPool;

use crate::{
    command::{
        color_setting_command, init_streams_command, led_setting_command,
        read_depth_params_command, read_firware_versions_command, read_p0_tables_command,
        read_rgb_params_command, read_serial_number_command, read_status_command, set_mode_command,
        set_stream_state_command, shutdown_command, stop_command, ColorSettingResponse,
        CommandTransaction,
    },
    data::{ColorParams, FirwareVersion, IrParams, P0Tables, PacketParams},
    packet::{
        parser::{DepthStreamParser, RgbStreamParser},
        DepthPacket, RgbPacket,
    },
    settings::{ColorSettingCommandType, LedSettings},
    Error, FromBuffer, ReadUnaligned, TIMEOUT,
};

use super::{Closed, Device, DeviceId, DeviceInfo};

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

pub struct Opened<C: UsbContext> {
    command_transaction: CommandTransaction<C>,
    device_handle: Arc<rusb::DeviceHandle<C>>,
    device: rusb::Device<C>,
    color_params: ColorParams,
    ir_params: IrParams,
    p0_tables: P0Tables,
    packet_params: PacketParams,
    rgb_transfer_pool: TransferPool<C>,
    rgb_stream_parser: RgbStreamParser,
    depth_transfer_pool: TransferPool<C>,
    depth_stream_parser: DepthStreamParser,
    running: bool,
}

impl<C: UsbContext> Opened<C> {
    pub(super) fn new(device: rusb::Device<C>) -> Result<Self, Error> {
        let device_handle = Arc::new(device.open()?);

        if device_handle.active_configuration()? != 1 {
            device_handle.set_active_configuration(1)?;
        }

        device_handle.claim_interface(InterfaceId::ControlAndRgb as u8)?;
        device_handle.claim_interface(InterfaceId::Ir as u8)?;

        // set isochronous delay
        device_handle.write_control(
            request_type(Direction::Out, RequestType::Standard, Recipient::Device),
            SET_ISOCH_DELAY,
            40,
            0,
            &[],
            TIMEOUT,
        )?;

        let mut opened_device = Self {
            command_transaction: CommandTransaction::new(
                CONTROL_IN_ENDPOINT,
                CONTROL_OUT_ENDPOINT,
                device_handle.clone(),
            ),
            device,
            color_params: Default::default(),
            ir_params: Default::default(),
            p0_tables: Default::default(),
            packet_params: Default::default(),
            rgb_transfer_pool: TransferPool::new(device_handle.clone())?,
            rgb_stream_parser: RgbStreamParser::new(),
            depth_transfer_pool: TransferPool::new(device_handle.clone())?,
            depth_stream_parser: DepthStreamParser::new(),
            running: false,
            device_handle,
        };

        // set power state latencies
        opened_device.set_sel(&[0x55, 0, 0x55, 0, 0, 0])?;
        opened_device.set_ir_state(false)?;
        // enable power states
        opened_device.set_feature(Feature::U1Enable)?;
        opened_device.set_feature(Feature::U2Enable)?;
        opened_device.set_video_transfer_function_state(false)?;
        // get ir max packet size
        opened_device.packet_params.max_iso_packet_size = opened_device
            .get_max_iso_packet_size(1, 1, IR_IN_ENDPOINT)
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

    fn set_sel(&self, data: &[u8]) -> Result<(), Error> {
        self.device_handle.write_control(
            request_type(Direction::Out, RequestType::Standard, Recipient::Device),
            REQUEST_SET_SEL,
            0,
            0,
            data,
            TIMEOUT,
        )?;

        Ok(())
    }

    fn set_feature(&self, feature: Feature) -> Result<(), Error> {
        self.device_handle.write_control(
            request_type(Direction::Out, RequestType::Standard, feature.recipient()),
            REQUEST_SET_FEATURE,
            feature as u16,
            0,
            &[],
            TIMEOUT,
        )?;

        Ok(())
    }

    fn set_feature_function_suspend(
        &self,
        low_power_suspend: bool,
        function_remote_wake: bool,
    ) -> Result<(), Error> {
        let feature = Feature::FunctionSuspend;
        let suspend_options = (low_power_suspend as u16) + ((function_remote_wake as u16) << 1);

        self.device_handle.write_control(
            request_type(Direction::Out, RequestType::Standard, feature.recipient()),
            REQUEST_SET_FEATURE,
            feature as u16,
            suspend_options << 8 | 0,
            &[],
            TIMEOUT,
        )?;

        Ok(())
    }

    fn get_max_iso_packet_size(
        &self,
        configuration_value: u8,
        alternate_setting_index: u8,
        endpoint_address: u8,
    ) -> Option<u16> {
        let device_descriptor = self.device.device_descriptor().ok()?;
        let configuration_descriptor = (0..device_descriptor.num_configurations())
            .filter_map(|configuration_index| {
                self.device.config_descriptor(configuration_index).ok()
            })
            .find(|configuration_descriptor| {
                configuration_descriptor.number() == configuration_value
            })?;

        for interface in configuration_descriptor.interfaces() {
            for interface_descriptor in interface.descriptors() {
                if interface_descriptor.setting_number() != alternate_setting_index {
                    continue;
                }

                for endpoint_descriptor in interface_descriptor.endpoint_descriptors() {
                    let Some(buffer) = endpoint_descriptor.extra() else {
                        continue;
                    };

                    if endpoint_descriptor.address() == endpoint_address
                        && endpoint_descriptor.transfer_type() == TransferType::Isochronous
                        && buffer[1] == DT_SS_ENDPOINT_COMPANION
                    {
                        return Some(u16::from_buffer(&buffer[4..6]));
                    }
                }
            }
        }

        None
    }

    fn set_ir_state(&mut self, enabled: bool) -> Result<(), Error> {
        Ok(self
            .device_handle
            .set_alternate_setting(InterfaceId::Ir as u8, enabled as u8)?)
    }

    fn set_video_transfer_function_state(&self, enabled: bool) -> Result<(), Error> {
        self.set_feature_function_suspend(!enabled, !enabled)
    }
}

impl<C: UsbContext> Device<Opened<C>> {
    pub fn running(&self) -> bool {
        self.inner.running
    }

    /// Start data processing with both RGB and depth streams.
    /// All above configuration must only be called before start() or after stop().
    pub fn start(&mut self) -> Result<(), Error> {
        if self.inner.running {
            return Ok(());
        }

        self.inner.running = true;

        self.inner.set_video_transfer_function_state(true)?;

        let usb_serial_number = self
            .inner
            .device_handle
            .read_serial_number_string_ascii(&self.inner.device.device_descriptor()?)
            .unwrap_or_default();
        let device_protocol_serial_number = self.get_serial_number()?;

        if device_protocol_serial_number != usb_serial_number {
            return Err(Error::SerialNumber(
                device_protocol_serial_number,
                usb_serial_number,
            ));
        }

        self.inner.ir_params = IrParams::try_from(
            self.inner
                .command_transaction
                .execute(read_depth_params_command())?
                .as_slice(),
        )?;
        self.inner.color_params = ColorParams::try_from(
            self.inner
                .command_transaction
                .execute(read_rgb_params_command())?
                .as_slice(),
        )?;
        self.inner.p0_tables = P0Tables::try_from(
            self.inner
                .command_transaction
                .execute(read_p0_tables_command())?
                .as_slice(),
        )?;

        self.inner
            .command_transaction
            .execute(set_mode_command(true, 0x00640064))?;
        self.inner
            .command_transaction
            .execute(set_mode_command(false, 0))?;

        for _ in 0..50 {
            if u32::from_buffer(
                &self
                    .inner
                    .command_transaction
                    .execute(read_status_command(0x090000))?,
            ) & 1
                != 0
            {
                break;
            }
            sleep(Duration::from_millis(100));
        }

        self.inner
            .command_transaction
            .execute(init_streams_command())?;
        self.inner.set_ir_state(true)?;
        self.inner
            .command_transaction
            .execute(set_stream_state_command(true))?;

        Ok(())
    }

    pub fn poll_rgb_frame(&mut self) -> Result<Option<RgbPacket>, Error> {
        if !self.inner.running {
            return Err(Error::OnlyWhileRunning("Reading rgb frame"));
        }

        for _ in 0..self.inner.packet_params.rgb_num_transfers {
            self.inner.rgb_transfer_pool.submit_bulk(
                RGB_IN_ENDPOINT,
                Vec::with_capacity(self.inner.packet_params.rgb_transfer_size),
            )?;
        }

        let mut result = None;

        while self.inner.rgb_transfer_pool.pending() {
            if let Some(packet) = self
                .inner
                .rgb_stream_parser
                .parse(self.inner.rgb_transfer_pool.poll(TIMEOUT)?)
            {
                result = Some(packet);
            }
        }

        Ok(result)
    }

    pub fn poll_depth_frame(&mut self) -> Result<Option<DepthPacket>, Error> {
        if !self.inner.running {
            return Err(Error::OnlyWhileRunning("Reading depth frame"));
        }

        for _ in 0..self.inner.packet_params.ir_num_transfers {
            self.inner.depth_transfer_pool.submit_iso(
                IR_IN_ENDPOINT,
                Vec::with_capacity(
                    self.inner.packet_params.ir_packets_per_transfer as usize
                        * self.inner.packet_params.max_iso_packet_size as usize,
                ),
                self.inner.packet_params.ir_packets_per_transfer,
            )?;
        }

        let mut result = None;

        while self.inner.depth_transfer_pool.pending() {
            if let Some(packet) = self
                .inner
                .depth_stream_parser
                .parse(self.inner.depth_transfer_pool.poll(TIMEOUT)?)
            {
                result = Some(packet);
            }
        }

        Ok(result)
    }

    pub fn get_firware_versions(&mut self) -> Result<Vec<FirwareVersion>, Error> {
        let buffer = self
            .inner
            .command_transaction
            .execute(read_firware_versions_command())?;
        const FIRWARE_VERSION_SIZE: usize = 16;
        let mut versions = Vec::new();

        for index in 0..buffer.len() / FIRWARE_VERSION_SIZE {
            versions.push(FirwareVersion::try_from(
                &buffer[index * FIRWARE_VERSION_SIZE..],
            )?);
        }

        Ok(versions)
    }

    pub fn get_serial_number(&mut self) -> Result<String, Error> {
        let mut buffer = self
            .inner
            .command_transaction
            .execute(read_serial_number_command())?;

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

    /// Sets the RGB camera to fully automatic exposure setting.
    /// Exposure compensation: negative value gives an underexposed image, positive gives an overexposed image.
    ///
    /// # Arguments
    ///
    /// * `exposure_compensation` - Exposure compensation, range [-2.0, 2.0]
    pub fn set_color_auto_exposure(&mut self, exposure_compensation: f32) -> Result<(), Error> {
        if !self.inner.running {
            return Err(Error::OnlyWhileRunning("Setting auto exposure"));
        }

        self.set_color_setting(ColorSettingCommandType::SetAcs, 0)?;
        self.set_color_setting(ColorSettingCommandType::SetExposureMode, 0)?;
        self.set_color_setting(
            ColorSettingCommandType::SetExposureCompensation,
            exposure_compensation.clamp(-2.0, 2.0).to_bits(),
        )?;

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
    pub fn set_color_semi_auto_exposure(
        &mut self,
        pseudo_exposure_time: Duration,
    ) -> Result<(), Error> {
        if !self.inner.running {
            return Err(Error::OnlyWhileRunning("Setting semi-auto exposure"));
        }

        self.set_color_setting(ColorSettingCommandType::SetAcs, 0)?;
        self.set_color_setting(ColorSettingCommandType::SetExposureMode, 3)?;
        self.set_color_setting(
            ColorSettingCommandType::SetExposureTimeMs,
            ((pseudo_exposure_time.as_secs_f64() / 1000.0) as f32)
                .clamp(0.0, 66.0)
                .to_bits(),
        )?;

        Ok(())
    }

    /// Manually set true exposure time and analog gain of the RGB camera.
    ///
    /// # Arguments
    ///
    /// * `integration_time` - True shutter time in milliseconds, range (0.0, 66.0]
    /// * `analog_gain` - Analog gain, range [1.0, 4.0]
    pub fn set_color_manual_exposure(
        &mut self,
        integration_time: Duration,
        analog_gain: f32,
    ) -> Result<(), Error> {
        if !self.inner.running {
            return Err(Error::OnlyWhileRunning("Setting manual exposure"));
        }

        self.set_color_setting(ColorSettingCommandType::SetAcs, 0)?;
        self.set_color_setting(ColorSettingCommandType::SetExposureMode, 4)?;
        self.set_color_setting(
            ColorSettingCommandType::SetIntegrationTime,
            ((integration_time.as_secs_f64() / 1000.0) as f32)
                .clamp(0.0, 66.0)
                .to_bits(),
        )?;
        self.set_color_setting(
            ColorSettingCommandType::SetAnalogGain,
            analog_gain.clamp(1.0, 4.0).to_bits(),
        )?;

        Ok(())
    }

    /// Set an individual setting value of the RGB camera.
    pub fn set_color_setting(
        &mut self,
        command: ColorSettingCommandType,
        value: u32,
    ) -> Result<(), Error> {
        self.inner
            .command_transaction
            .execute(color_setting_command(command, value))?;

        Ok(())
    }

    /// get an individual setting value of the RGB camera.
    pub fn get_color_setting(&mut self, command: ColorSettingCommandType) -> Result<u32, Error> {
        let bytes = self
            .inner
            .command_transaction
            .execute(color_setting_command(command, 0))?;

        Ok(ColorSettingResponse::read_unaligned(&bytes)?.data)
    }

    /// Set the settings of a Kinect LED.
    ///
    /// # Arguments
    ///
    /// * `led_settings` - Settings for a single LED.
    pub fn set_led_status(&mut self, led_settings: LedSettings) -> Result<(), Error> {
        self.inner
            .command_transaction
            .execute(led_setting_command(led_settings))?;

        Ok(())
    }

    /// Stop data processing.
    pub fn stop(&mut self) -> Result<(), Error> {
        if !self.inner.running {
            return Ok(());
        }

        self.inner.running = false;

        self.inner.set_ir_state(false)?;
        self.inner
            .command_transaction
            .execute(set_mode_command(true, 0x00640064))?;
        self.inner
            .command_transaction
            .execute(set_mode_command(false, 0))?;
        self.inner.command_transaction.execute(stop_command())?;
        self.inner
            .command_transaction
            .execute(set_stream_state_command(false))?;
        self.inner
            .command_transaction
            .execute(set_mode_command(true, 0))?;
        self.inner
            .command_transaction
            .execute(set_mode_command(false, 0))?;
        self.inner
            .command_transaction
            .execute(set_mode_command(true, 0))?;
        self.inner
            .command_transaction
            .execute(set_mode_command(false, 0))?;
        self.inner.set_video_transfer_function_state(false)
    }

    /// Shut down the device.
    pub fn close(mut self) -> Result<Device<Closed<C>>, Error> {
        self.stop()?;
        self.inner
            .command_transaction
            .execute(set_mode_command(true, 0x00640064))?;
        self.inner
            .command_transaction
            .execute(set_mode_command(false, 0))?;
        self.inner.command_transaction.execute(shutdown_command())?;

        Ok(Device {
            inner: Closed {
                device: self.inner.device,
            },
        })
    }
}

impl<C: UsbContext> DeviceInfo for Device<Opened<C>> {
    fn id(&self) -> DeviceId {
        DeviceId {
            bus: self.inner.device.bus_number(),
            address: self.inner.device.address(),
        }
    }
}

impl<C: UsbContext> Debug for Device<Opened<C>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.device.fmt(f)
    }
}

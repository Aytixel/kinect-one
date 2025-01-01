use crate::{config::LedSettings, settings::ColorSettingCommandType};

use super::{
    ColorSettingResponse, Command, DepthParamsResponse, P0TablesResponse, RgbParamsResponse,
};

// Kinect commands id
const KINECT_CMD_READ_FIRMWARE_VERSIONS: u32 = 0x02;
const KINECT_CMD_INIT_STREAMS: u32 = 0x09;
const KINECT_CMD_READ_HARDWARE_INFO: u32 = 0x14;
const KINECT_CMD_READ_STATUS: u32 = 0x16;
const KINECT_CMD_READ_DATA_PAGE: u32 = 0x22;

const KINECT_CMD_SET_STREAMING: u32 = 0x2b;
const KINECT_CMD_SET_MODE: u32 = 0x4b;

const KINECT_CMD_RGB_SETTING: u32 = 0x3e;

const KINECT_CMD_STOP: u32 = 0x0a;
const KINECT_CMD_SHUTDOWN: u32 = 0x00;

// Response size
const P0_TABLES_RESPONSE_SIZE: u32 = size_of::<P0TablesResponse>() as u32;
const DEPTH_PARAMS_RESPONSE_SIZE: u32 = size_of::<DepthParamsResponse>() as u32;
const RGB_PARAMS_RESPONSE_SIZE: u32 = size_of::<RgbParamsResponse>() as u32;
const COLOR_SETTING_RESPONSE_SIZE: u32 = size_of::<ColorSettingResponse>() as u32;

pub fn read_firware_versions_command() -> Command<KINECT_CMD_READ_FIRMWARE_VERSIONS, 0x200, 0x200, 0>
{
    Command {
        has_sequence: true,
        parameters: [],
    }
}

pub fn read_hardware_info_command() -> Command<KINECT_CMD_READ_HARDWARE_INFO, 0x5c, 0x5c, 0> {
    Command {
        has_sequence: true,
        parameters: [],
    }
}

pub fn init_streams_command() -> Command<KINECT_CMD_INIT_STREAMS, 0, 0, 0> {
    Command {
        has_sequence: true,
        parameters: [],
    }
}

pub fn read_serial_number_command() -> Command<KINECT_CMD_READ_DATA_PAGE, 0x80, 0x80, 1> {
    Command {
        has_sequence: true,
        parameters: [0x01],
    }
}

pub fn read_p0_tables_command(
) -> Command<KINECT_CMD_READ_DATA_PAGE, 0x1C0000, P0_TABLES_RESPONSE_SIZE, 1> {
    Command {
        has_sequence: true,
        parameters: [0x02],
    }
}

pub fn read_depth_params_command(
) -> Command<KINECT_CMD_READ_DATA_PAGE, 0x1C0000, DEPTH_PARAMS_RESPONSE_SIZE, 1> {
    Command {
        has_sequence: true,
        parameters: [0x03],
    }
}

pub fn read_rgb_params_command(
) -> Command<KINECT_CMD_READ_DATA_PAGE, 0x1C0000, RGB_PARAMS_RESPONSE_SIZE, 1> {
    Command {
        has_sequence: true,
        parameters: [0x04],
    }
}

pub fn read_status_command(status: u32) -> Command<KINECT_CMD_READ_STATUS, 0x04, 0x04, 1> {
    Command {
        has_sequence: true,
        parameters: [status],
    }
}

pub fn set_stream_state_command(enabled: bool) -> Command<KINECT_CMD_SET_STREAMING, 0, 0, 1> {
    Command {
        has_sequence: true,
        parameters: [enabled as u32],
    }
}

pub fn stop_command() -> Command<KINECT_CMD_STOP, 0, 0, 0> {
    Command {
        has_sequence: true,
        parameters: [],
    }
}

pub fn shutdown_command() -> Command<KINECT_CMD_SHUTDOWN, 0, 0, 0> {
    Command {
        has_sequence: true,
        parameters: [],
    }
}

pub fn set_mode_command(enabled: bool, mode: u32) -> Command<KINECT_CMD_SET_MODE, 0, 0, 4> {
    Command {
        has_sequence: true,
        parameters: [enabled as u32, mode, 0, 0],
    }
}

pub fn color_setting_command(
    command: ColorSettingCommandType,
    value: u32,
) -> Command<KINECT_CMD_RGB_SETTING, COLOR_SETTING_RESPONSE_SIZE, COLOR_SETTING_RESPONSE_SIZE, 4> {
    Command {
        has_sequence: false,
        parameters: [1, 0, command as u32, value],
    }
}

pub fn led_setting_command(led_settings: LedSettings) -> Command<KINECT_CMD_SET_MODE, 0, 0, 4> {
    Command {
        has_sequence: false,
        parameters: [
            (led_settings.id() as u16 as u32)
                + (led_settings.mode() as u16 as u32).overflowing_shl(16).0,
            (led_settings.start_level() as u32)
                + (led_settings.stop_level() as u32).overflowing_shl(16).0,
            led_settings.interval().as_millis() as u32,
            0,
        ],
    }
}

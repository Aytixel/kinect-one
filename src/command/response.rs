use crate::{config::DEPTH_FRAME_SIZE, ReadUnaligned};

// probably some combination of color camera intrinsics + depth coefficient tables
#[repr(C, packed)]
pub struct RgbParamsResponse {
    // unknown, always seen as 1 so far
    _table_id: u8,

    // color -> depth mapping parameters
    pub color_f: f32,
    pub color_cx: f32,
    pub color_cy: f32,

    pub shift_d: f32,
    pub shift_m: f32,

    // xxx
    pub mx_x3y0: f32,
    // yyy
    pub mx_x0y3: f32,
    // xxy
    pub mx_x2y1: f32,
    // yyx
    pub mx_x1y2: f32,
    // xx
    pub mx_x2y0: f32,
    // yy
    pub mx_x0y2: f32,
    // xy
    pub mx_x1y1: f32,
    // x
    pub mx_x1y0: f32,
    // y
    pub mx_x0y1: f32,
    // 1
    pub mx_x0y0: f32,

    // xxx
    pub my_x3y0: f32,
    // yyy
    pub my_x0y3: f32,
    // xxy
    pub my_x2y1: f32,
    // yyx
    pub my_x1y2: f32,
    // xx
    pub my_x2y0: f32,
    // yy
    pub my_x0y2: f32,
    // xy
    pub my_x1y1: f32,
    // x
    pub my_x1y0: f32,
    // y
    pub my_x0y1: f32,
    // 1
    pub my_x0y0: f32,

    // perhaps related to xtable/ztable in the deconvolution code.
    // data seems to be arranged into two tables of 28*23, which
    // matches the depth image aspect ratio of 512*424 very closely
    _table1: [f32; 28 * 23 * 4],
    _table2: [f32; 28 * 23],
}

impl ReadUnaligned for RgbParamsResponse {}

// depth camera intrinsic & distortion parameters
#[repr(C, packed)]
pub struct DepthParamsResponse {
    // intrinsics (this is pretty certain)
    pub fx: f32,
    pub fy: f32,
    // assumed to be always zero
    _unknown0: f32,
    pub cx: f32,
    pub cy: f32,
    // radial distortion (educated guess based on calibration data from Kinect SDK)
    pub k1: f32,
    pub k2: f32,
    // always seen as zero so far, so purely a guess
    pub p1: f32,
    // always seen as zero so far, so purely a guess
    pub p2: f32,
    pub k3: f32,
    // assumed to be always zero
    _unknown1: [f32; 13],
}

impl ReadUnaligned for DepthParamsResponse {}

// "P0" coefficient tables, input to the deconvolution code
#[repr(C, packed)]
pub struct P0TablesResponse {
    _headersize: u32,
    _unknown0: u32,
    _unknown1: u32,
    _tablesize: u32,
    _unknown2: u32,
    _unknown3: u32,
    _unknown4: u32,
    _unknown5: u32,

    _unknown6: u16,
    // row[0] == row[511] == 0x2c9a
    pub p0_table0: [u16; DEPTH_FRAME_SIZE],
    _unknown7: u16,

    _unknown8: u16,
    // row[0] == row[511] == 0x08ec
    pub p0_table1: [u16; DEPTH_FRAME_SIZE],
    _unknown9: u16,

    _unknown10: u16,
    // row[0] == row[511] == 0x42e8
    pub p0_table2: [u16; DEPTH_FRAME_SIZE],
    _unknown11: u16,
}

impl ReadUnaligned for P0TablesResponse {}

#[repr(C, packed)]
pub struct FirmwareVersionResponse {
    pub min: u16,
    pub maj: u16,
    pub revision: u32,
    pub build: u32,
    _reserved0: u32,
}

impl ReadUnaligned for FirmwareVersionResponse {}

// RGB camera settings reply for a single setting change.
// Equivalent of NUISENSOR_RGB_CHANGE_STREAM_SETTING_REPLY in NuiSensorLib.h
#[repr(C, packed)]
pub struct ColorSettingResponse {
    _num_status: u32,
    _command_list_status: u32,
    // Result of the first command -- we only send one at a time for now.
    // Equivalent of a fixed-length array of NUISENSOR_RGB_CHANGE_STREAM_SETTING_REPLY_STATUS in NuiSensorLib.h
    _status: u32,
    pub data: u32,
}

impl ReadUnaligned for ColorSettingResponse {}

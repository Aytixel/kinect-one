const P0_TABLE_SIZE: usize = 512 * 424;

// "P0" coefficient tables, input to the deconvolution code
pub struct P0TablesResponse {
    pub header_size: u32,
    pub table_size: u32,
    // row[0] == row[511] == 0x2c9a
    pub p0_table0: [u16; P0_TABLE_SIZE],
    // row[0] == row[511] == 0x08ec
    pub p0_table1: [u16; P0_TABLE_SIZE],
    // row[0] == row[511] == 0x42e8
    pub p0_table2: [u16; P0_TABLE_SIZE],
}

// RGB camera settings reply for a single setting change.
// Equivalent of NUISENSOR_RGB_CHANGE_STREAM_SETTING_REPLY in NuiSensorLib.h
pub struct ColorSettingResponse {
    pub num_status: u32,
    pub command_list_status: u32,

    // Result of the first command -- we only send one at a time for now.
    // Equivalent of a fixed-length array of NUISENSOR_RGB_CHANGE_STREAM_SETTING_REPLY_STATUS in NuiSensorLib.h
    pub status: u32,
    pub data: u32,
}

use crate::{
    config::{Config, DEPTH_FRAME_SIZE},
    data::P0Tables,
};

pub use crate::packet::DepthPacket;

pub trait DepthProcessorTrait {
    fn set_config(config: &Config);

    fn set_p0_tables(p0_tables: &P0Tables);

    fn set_x_z_tables(xtable: &[f32; DEPTH_FRAME_SIZE], ztable: &[f32; DEPTH_FRAME_SIZE]);

    fn set_lookup_table(lut: u16);
}

#[cfg(feature = "cpu_depth")]
mod cpu;
#[cfg(feature = "opencl_depth")]
mod opencl;
#[cfg(feature = "opencl_kde_depth")]
mod opencl_kde;

use std::{error::Error, f32::EPSILON};

#[cfg(feature = "cpu_depth")]
pub use cpu::*;
#[cfg(feature = "opencl_depth")]
pub use opencl::*;
#[cfg(feature = "opencl_kde_depth")]
pub use opencl_kde::*;

use crate::{
    config::Config,
    data::{IrParams, P0Tables},
    LUT_SIZE, TABLE_SIZE,
};

pub use crate::packet::DepthPacket;

#[derive(Debug, Clone)]
pub struct DepthFrame {
    pub width: usize,
    pub height: usize,
    pub buffer: Box<[f32; TABLE_SIZE]>,

    pub sequence: u32,
    pub timestamp: u32,
}

pub type IrFrame = DepthFrame;

pub trait DepthProcessorTrait {
    fn set_config(&mut self, config: &Config) -> Result<(), Box<dyn Error>>;

    fn set_p0_tables(&mut self, p0_tables: &P0Tables) -> Result<(), Box<dyn Error>>;

    fn set_x_z_tables(
        &mut self,
        x_table: &[f32; TABLE_SIZE],
        z_table: &[f32; TABLE_SIZE],
    ) -> Result<(), Box<dyn Error>>;

    fn set_lookup_table(&mut self, lut: &[i16; LUT_SIZE]) -> Result<(), Box<dyn Error>>;

    fn set_ir_params(&mut self, ir_params: &IrParams) -> Result<(), Box<dyn Error>> {
        let mut x_table = [0.0; TABLE_SIZE];
        let mut z_table = [0.0; TABLE_SIZE];
        let mut lut = [0; LUT_SIZE];

        const SCALING_FACTOR: f32 = 8192.0;
        const UNAMBIGUOUS_DIST: f32 = 6250.0 / 3.0;

        for i in 0..TABLE_SIZE {
            let xi = i % 512;
            let yi = i / 512;
            let xd = (xi as f32 + 0.5 - ir_params.cx) / ir_params.fx;
            let yd = (yi as f32 + 0.5 - ir_params.cy) / ir_params.fy;

            let (xu, yu) = Self::undistort(ir_params, xd, yd);

            x_table[i] = SCALING_FACTOR * xu;
            z_table[i] = UNAMBIGUOUS_DIST / (xu * xu + yu * yu + 1.0).sqrt();
        }

        let mut y = 0;

        for x in 0..1024 {
            let inc = 1 << (x / 128 - (x >= 128) as usize);

            lut[x] = y;
            lut[1024 + x] = -y;
            y += inc;
        }

        lut[1024] = 32767;

        self.set_x_z_tables(&x_table, &z_table)?;
        self.set_lookup_table(&lut)?;

        Ok(())
    }

    fn distort(ir_params: &IrParams, x: f32, y: f32) -> (f32, f32) {
        let x2 = x * x;
        let y2 = y * y;
        let r2 = x2 + y2;
        let xy = x * y;
        let kr = ((ir_params.k3 * r2 + ir_params.k2) * r2 + ir_params.k1) * r2 + 1.0;

        (
            x * kr + ir_params.p2 * (r2 + 2.0 * x2) + 2.0 * ir_params.p1 * xy,
            y * kr + ir_params.p1 * (r2 + 2.0 * y2) + 2.0 * ir_params.p2 * xy,
        )
    }

    fn undistort(ir_params: &IrParams, mut x: f32, mut y: f32) -> (f32, f32) {
        let x0 = x;
        let y0 = y;

        let mut last_x = x;
        let mut last_y = y;

        for _ in 0..100 {
            let x2 = x * x;
            let y2 = y * y;
            let x2y2 = x2 + y2;
            let x2y22 = x2y2 * x2y2;
            let x2y23 = x2y2 * x2y22;

            // Jacobian matrix
            let ja = ir_params.k3 * x2y23
                + (ir_params.k2 + 6.0 * ir_params.k3 * x2) * x2y22
                + (ir_params.k1 + 4.0 * ir_params.k2 * x2) * x2y2
                + 2.0 * ir_params.k1 * x2
                + 6.0 * ir_params.p2 * x
                + 2.0 * ir_params.p1 * y
                + 1.0;
            let jb = 6.0 * ir_params.k3 * x * y * x2y22
                + 4.0 * ir_params.k2 * x * y * x2y2
                + 2.0 * ir_params.k1 * x * y
                + 2.0 * ir_params.p1 * x
                + 2.0 * ir_params.p2 * y;
            let jc = jb;
            let jd = ir_params.k3 * x2y23
                + (ir_params.k2 + 6.0 * ir_params.k3 * y2) * x2y22
                + (ir_params.k1 + 4.0 * ir_params.k2 * y2) * x2y2
                + 2.0 * ir_params.k1 * y2
                + 2.0 * ir_params.p2 * x
                + 6.0 * ir_params.p1 * y
                + 1.0;

            // Inverse jacobian
            let jdet = 1.0 / (ja * jd - jb * jc);
            let a = jd * jdet;
            let b = -jb * jdet;
            let c = -jc * jdet;
            let d = ja * jdet;

            let (mut f, mut g) = Self::distort(ir_params, x, y);

            f -= x0;
            g -= y0;

            x -= a * f + b * g;
            y -= c * f + d * g;

            const EPS: f32 = EPSILON * 16.0;

            if (x - last_x).abs() <= EPS && (y - last_y).abs() <= EPS {
                break;
            }

            last_x = x;
            last_y = y;
        }

        (x, y)
    }
}

use std::{fmt, ptr::read_unaligned};

use crate::{
    command::{DepthParamsResponse, FirmwareVersionResponse, P0TablesResponse, RgbParamsResponse},
    config::DEPTH_FRAME_SIZE,
    Error, ReadUnaligned,
};

#[derive(Debug, Clone, Copy)]
/// Parameters of depth processing.
pub struct DepthProcessorParams {
    pub ab_multiplier: f32,
    pub ab_multiplier_per_frq: [f32; 3],
    pub ab_output_multiplier: f32,

    pub phase_in_rad: [f32; 3],

    pub joint_bilateral_ab_threshold: f32,
    pub joint_bilateral_max_edge: f32,
    pub joint_bilateral_exp: f32,
    pub gaussian_kernel: [f32; 9],

    pub phase_offset: f32,
    pub unambigious_dist: f32,
    pub individual_ab_threshold: f32,
    pub ab_threshold: f32,
    pub ab_confidence_slope: f32,
    pub ab_confidence_offset: f32,
    pub min_dealias_confidence: f32,
    pub max_dealias_confidence: f32,

    pub edge_ab_avg_min_value: f32,
    pub edge_ab_std_dev_threshold: f32,
    pub edge_close_delta_threshold: f32,
    pub edge_far_delta_threshold: f32,
    pub edge_max_delta_threshold: f32,
    pub edge_avg_delta_threshold: f32,
    pub max_edge_count: f32,

    pub kde_sigma_sqr: f32,
    pub unwrapping_likelihood_scale: f32,
    pub phase_confidence_scale: f32,
    pub kde_threshold: f32,
    pub kde_neigborhood_size: usize,
    pub num_hyps: usize,

    pub min_depth: f32,
    pub max_depth: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct PacketParams {
    pub max_iso_packet_size: u16,
    pub rgb_transfer_size: usize,
    pub rgb_num_transfers: usize,
    pub ir_packets_per_transfer: usize,
    pub ir_num_transfers: usize,
}

impl Default for PacketParams {
    fn default() -> Self {
        if cfg!(target_os = "macos") {
            Self {
                max_iso_packet_size: 0,
                rgb_transfer_size: 0x4000,
                rgb_num_transfers: 20,
                ir_packets_per_transfer: 128,
                ir_num_transfers: 4,
            }
        } else if cfg!(target_os = "windows") {
            Self {
                max_iso_packet_size: 0,
                rgb_transfer_size: 1048576,
                rgb_num_transfers: 3,
                ir_packets_per_transfer: 64,
                ir_num_transfers: 8,
            }
        } else {
            Self {
                max_iso_packet_size: 0,
                rgb_transfer_size: 0x4000,
                rgb_num_transfers: 20,
                ir_packets_per_transfer: 8,
                ir_num_transfers: 60,
            }
        }
    }
}

/// Color camera calibration parameters.
/// Kinect v2 includes factory preset values for these parameters.
/// They are used in Registration.
#[derive(Debug, Default, Clone, Copy)]
pub struct ColorParams {
    /*
        Intrinsic parameters
    */
    /// Focal length x (pixel)
    pub fx: f32,
    /// Focal length y (pixel)
    pub fy: f32,
    /// Principal point x (pixel)
    pub cx: f32,
    /// Principal point y (pixel)
    pub cy: f32,

    /*
        Extrinsic parameters

        These parameters are used in [a formula](https://github.com/OpenKinect/libfreenect2/issues/41#issuecomment-72022111)
        to map coordinates in the depth camera to the color camera.

        They cannot be used for matrix transformation.
    */
    pub shift_d: f32,
    pub shift_m: f32,

    pub mx_x3y0: f32,
    pub mx_x0y3: f32,
    pub mx_x2y1: f32,
    pub mx_x1y2: f32,
    pub mx_x2y0: f32,
    pub mx_x0y2: f32,
    pub mx_x1y1: f32,
    pub mx_x1y0: f32,
    pub mx_x0y1: f32,
    pub mx_x0y0: f32,

    pub my_x3y0: f32,
    pub my_x0y3: f32,
    pub my_x2y1: f32,
    pub my_x1y2: f32,
    pub my_x2y0: f32,
    pub my_x0y2: f32,
    pub my_x1y1: f32,
    pub my_x1y0: f32,
    pub my_x0y1: f32,
    pub my_x0y0: f32,
}

impl TryFrom<&[u8]> for ColorParams {
    type Error = Error;

    fn try_from(buffer: &[u8]) -> Result<Self, Self::Error> {
        let raw = RgbParamsResponse::read_unaligned(buffer)?;

        Ok(Self {
            fx: raw.color_f,
            fy: raw.color_f,
            cx: raw.color_cx,
            cy: raw.color_cy,
            shift_d: raw.shift_d,
            shift_m: raw.shift_m,
            mx_x3y0: raw.mx_x3y0,
            mx_x0y3: raw.mx_x0y3,
            mx_x2y1: raw.mx_x2y1,
            mx_x1y2: raw.mx_x1y2,
            mx_x2y0: raw.mx_x2y0,
            mx_x0y2: raw.mx_x0y2,
            mx_x1y1: raw.mx_x1y1,
            mx_x1y0: raw.mx_x1y0,
            mx_x0y1: raw.mx_x0y1,
            mx_x0y0: raw.mx_x0y0,
            my_x3y0: raw.my_x3y0,
            my_x0y3: raw.my_x0y3,
            my_x2y1: raw.my_x2y1,
            my_x1y2: raw.my_x1y2,
            my_x2y0: raw.my_x2y0,
            my_x0y2: raw.my_x0y2,
            my_x1y1: raw.my_x1y1,
            my_x1y0: raw.my_x1y0,
            my_x0y1: raw.my_x0y1,
            my_x0y0: raw.my_x0y0,
        })
    }
}

/// IR camera intrinsic calibration parameters.
/// Kinect v2 includes factory preset values for these parameters.
/// They are used in depth image decoding, and Registration.
#[derive(Debug, Default, Clone, Copy)]
pub struct IrParams {
    /// Focal length x (pixel)
    pub fx: f32,
    /// Focal length y (pixel)
    pub fy: f32,
    /// Principal point x (pixel)
    pub cx: f32,
    /// Principal point y (pixel)
    pub cy: f32,
    /// Radial distortion coefficient, 1st-order
    pub k1: f32,
    /// Radial distortion coefficient, 2nd-order
    pub k2: f32,
    /// Radial distortion coefficient, 3rd-order
    pub k3: f32,
    /// Tangential distortion coefficient
    pub p1: f32,
    /// Tangential distortion coefficient
    pub p2: f32,
}

impl TryFrom<&[u8]> for IrParams {
    type Error = Error;

    fn try_from(buffer: &[u8]) -> Result<Self, Self::Error> {
        let raw = DepthParamsResponse::read_unaligned(buffer)?;

        Ok(Self {
            fx: raw.fx,
            fy: raw.fy,
            cx: raw.cx,
            cy: raw.cy,
            k1: raw.k1,
            k2: raw.k2,
            k3: raw.k3,
            p1: raw.p1,
            p2: raw.p2,
        })
    }
}

pub type P0Table = [u16; DEPTH_FRAME_SIZE];

#[derive(Debug, Clone)]
pub struct P0Tables {
    pub p0_table0: Box<P0Table>,
    pub p0_table1: Box<P0Table>,
    pub p0_table2: Box<P0Table>,
}

impl TryFrom<&[u8]> for P0Tables {
    type Error = Error;

    fn try_from(buffer: &[u8]) -> Result<Self, Self::Error> {
        if buffer.len() < P0TablesResponse::size() {
            return Err(Error::UnalignedRead("P0TablesResponse"));
        }

        let raw = unsafe { read_unaligned(buffer.as_ptr() as *const P0TablesResponse) };

        Ok(Self {
            p0_table0: Box::new(raw.p0_table0),
            p0_table1: Box::new(raw.p0_table1),
            p0_table2: Box::new(raw.p0_table2),
        })
    }
}

impl Default for P0Tables {
    fn default() -> Self {
        Self {
            p0_table0: Box::new([0u16; DEPTH_FRAME_SIZE]),
            p0_table1: Box::new([0u16; DEPTH_FRAME_SIZE]),
            p0_table2: Box::new([0u16; DEPTH_FRAME_SIZE]),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FirwareVersion {
    pub maj: u16,
    pub min: u16,
    pub revision: u32,
    pub build: u32,
}

impl fmt::Display for FirwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{}.{}.{}.{}",
            self.maj, self.min, self.revision, self.build
        ))
    }
}

impl TryFrom<&[u8]> for FirwareVersion {
    type Error = Error;

    fn try_from(buffer: &[u8]) -> Result<Self, Self::Error> {
        let raw = FirmwareVersionResponse::read_unaligned(buffer)?;

        Ok(Self {
            maj: raw.maj,
            min: raw.min,
            revision: raw.revision,
            build: raw.build,
        })
    }
}

use std::fmt;

use crate::FromBuffer;

/// Color camera calibration parameters.
/// Kinect v2 includes factory preset values for these parameters.
/// They are used in Registration.
#[derive(Debug, Default, Clone)]
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

impl From<&[u8]> for ColorParams {
    fn from(buffer: &[u8]) -> Self {
        Self {
            fx: f32::from_buffer(&buffer[1..5]),
            fy: f32::from_buffer(&buffer[5..9]),
            cx: f32::from_buffer(&buffer[9..13]),
            cy: f32::from_buffer(&buffer[13..17]),
            shift_d: f32::from_buffer(&buffer[17..21]),
            shift_m: f32::from_buffer(&buffer[21..25]),
            mx_x3y0: f32::from_buffer(&buffer[25..29]),
            mx_x0y3: f32::from_buffer(&buffer[29..33]),
            mx_x2y1: f32::from_buffer(&buffer[33..37]),
            mx_x1y2: f32::from_buffer(&buffer[37..41]),
            mx_x2y0: f32::from_buffer(&buffer[41..45]),
            mx_x0y2: f32::from_buffer(&buffer[45..49]),
            mx_x1y1: f32::from_buffer(&buffer[49..53]),
            mx_x1y0: f32::from_buffer(&buffer[53..57]),
            mx_x0y1: f32::from_buffer(&buffer[57..61]),
            mx_x0y0: f32::from_buffer(&buffer[61..65]),
            my_x3y0: f32::from_buffer(&buffer[65..69]),
            my_x0y3: f32::from_buffer(&buffer[69..73]),
            my_x2y1: f32::from_buffer(&buffer[73..77]),
            my_x1y2: f32::from_buffer(&buffer[77..81]),
            my_x2y0: f32::from_buffer(&buffer[81..85]),
            my_x0y2: f32::from_buffer(&buffer[85..89]),
            my_x1y1: f32::from_buffer(&buffer[89..93]),
            my_x1y0: f32::from_buffer(&buffer[93..97]),
            my_x0y1: f32::from_buffer(&buffer[97..101]),
            my_x0y0: f32::from_buffer(&buffer[101..105]),
        }
    }
}

/// IR camera intrinsic calibration parameters.
/// Kinect v2 includes factory preset values for these parameters.
/// They are used in depth image decoding, and Registration.
#[derive(Debug, Default, Clone)]
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

impl From<&[u8]> for IrParams {
    fn from(buffer: &[u8]) -> Self {
        Self {
            fx: f32::from_buffer(&buffer[0..4]),
            fy: f32::from_buffer(&buffer[4..8]),
            cx: f32::from_buffer(&buffer[12..16]),
            cy: f32::from_buffer(&buffer[16..20]),
            k1: f32::from_buffer(&buffer[20..24]),
            k2: f32::from_buffer(&buffer[24..28]),
            k3: f32::from_buffer(&buffer[28..32]),
            p1: f32::from_buffer(&buffer[36..40]),
            p2: f32::from_buffer(&buffer[40..44]),
        }
    }
}

#[derive(Debug)]
pub struct FirwareVersion {
    pub maj: u16,
    pub min: u16,
    pub revision: u32,
    pub build: u32,
}

impl From<&[u8]> for FirwareVersion {
    fn from(buffer: &[u8]) -> Self {
        let maj_min = u32::from_buffer(&buffer[0..4]);
        let revision = u32::from_buffer(&buffer[4..8]);
        let build = u32::from_buffer(&buffer[8..12]);

        Self {
            maj: (maj_min >> 16) as u16,
            min: (maj_min & 0xffff) as u16,
            revision,
            build,
        }
    }
}

impl fmt::Display for FirwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{}.{}.{}.{}",
            self.maj, self.min, self.revision, self.build
        ))
    }
}

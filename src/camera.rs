/// Color camera calibration parameters.
/// Kinect v2 includes factory preset values for these parameters. They are used in Registration.
#[derive(Default, Clone)]
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

/// IR camera intrinsic calibration parameters.
/// Kinect v2 includes factory preset values for these parameters. They are used in depth image decoding, and Registration.
#[derive(Default, Clone)]
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

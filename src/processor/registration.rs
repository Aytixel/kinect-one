use std::f32::{INFINITY, NAN};

use crate::{
    data::{ColorParams, IrParams},
    COLOR_SIZE, COLOR_WIDTH, DEPTH_HEIGHT, DEPTH_SIZE, DEPTH_WIDTH,
};

use super::{color::ColorFrame, depth::DepthFrame};

const FILTER_WIDTH_HALF: isize = 2;
const FILTER_HEIGHT_HALF: isize = 1;
const FILTER_TOLERANCE: f32 = 0.01;

// these seem to be hardcoded in the original SDK
const DEPTH_Q: f32 = 0.01;
const COLOR_Q: f32 = 0.002199;

/// Registration will only work contiguous color space
pub struct Registration {
    /// Depth camera parameters.
    ir_params: IrParams,
    /// Color camera parameters.
    color_params: ColorParams,
    distort_map: Box<[usize; DEPTH_SIZE]>,
    depth_to_color_map_x: Box<[f32; DEPTH_SIZE]>,
    depth_to_color_map_y: Box<[f32; DEPTH_SIZE]>,
    depth_to_color_map_yi: Box<[usize; DEPTH_SIZE]>,
}

impl Registration {
    pub fn new() -> Self {
        Self {
            ir_params: Default::default(),
            color_params: Default::default(),
            distort_map: Box::new([0; DEPTH_SIZE]),
            depth_to_color_map_x: Box::new([0.0; DEPTH_SIZE]),
            depth_to_color_map_y: Box::new([0.0; DEPTH_SIZE]),
            depth_to_color_map_yi: Box::new([0; DEPTH_SIZE]),
        }
    }

    fn fill_depth_to_color_map(&mut self) {
        for y in 0..DEPTH_HEIGHT {
            for x in 0..DEPTH_WIDTH {
                let offset = x + y * DEPTH_WIDTH;

                // compute the dirstored coordinate for current pixel
                let (mx, my) = self.distort(x, y);
                // rounding the values and check if the pixel is inside the image
                let ix = (mx + 0.5) as u32;
                let iy = (my + 0.5) as u32;

                // computing the index from the coordianted for faster access to the data
                self.distort_map[offset] = iy as usize * DEPTH_WIDTH + ix as usize;

                // compute the depth to color mapping entries for the current pixel
                let (rx, ry) = self.depth_to_color(x as f32, y as f32);

                self.depth_to_color_map_x[offset] = rx;
                self.depth_to_color_map_y[offset] = ry;
                // compute the y offset to minimize later computations
                self.depth_to_color_map_yi[offset] = (ry + 0.5) as usize;
            }
        }
    }

    pub fn set_ir_params(&mut self, ir_params: &IrParams) {
        self.ir_params = *ir_params;
        self.fill_depth_to_color_map();
    }

    pub fn set_color_params(&mut self, color_params: &ColorParams) {
        self.color_params = *color_params;
        self.fill_depth_to_color_map();
    }

    pub fn undistort_depth_and_color(
        &self,
        color_frame: &ColorFrame,
        depth_frame: &DepthFrame,
        enable_filter: bool,
    ) -> (ColorFrame, DepthFrame) {
        let bytes_per_pixel = color_frame.color_space.bytes_per_pixel();
        let mut registered_frame = ColorFrame {
            color_space: color_frame.color_space,
            width: DEPTH_WIDTH,
            height: DEPTH_HEIGHT,
            buffer: vec![0; DEPTH_SIZE * bytes_per_pixel],
            sequence: color_frame.sequence,
            timestamp: color_frame.timestamp,
            exposure: color_frame.exposure,
            gain: color_frame.gain,
            gamma: color_frame.gamma,
        };
        let mut undistorted_frame = DepthFrame {
            width: DEPTH_WIDTH,
            height: DEPTH_HEIGHT,
            buffer: Vec::with_capacity(DEPTH_SIZE),
            sequence: depth_frame.sequence,
            timestamp: depth_frame.timestamp,
        };

        // map for storing the min z values used for each color pixel
        // initializing the depth_map with values outside of the Kinect2 range if filter is enabled
        let mut filter_map = [INFINITY; COLOR_SIZE];

        // map for storing the color offset for each depth pixel
        let mut depth_to_c_off = Vec::with_capacity(DEPTH_SIZE);

        /* Fix depth distortion, and compute pixel to use from 'color' based on depth measurement,
         * stored as x/y offset in the color data.
         */

        // iterating over all pixels from undistorted depth and registered color image
        // the four maps have the same structure as the images, so their pointers are increased each iteration as well
        for i in 0..DEPTH_SIZE {
            // getting index of distorted depth pixel
            let index = self.distort_map[i];

            // getting depth value for current pixel
            let z = depth_frame.buffer[index];

            undistorted_frame.buffer.push(z);

            // checking for invalid depth value
            if z <= 0.0 {
                depth_to_c_off.push(None);
                continue;
            }

            // calculating x offset for color image based on depth value
            let cx = ((self.depth_to_color_map_x[i] + (self.color_params.shift_m / z))
                * self.color_params.fx
                + self.color_params.cx.round()) as usize;
            // getting y offset for depth image
            let cy = self.depth_to_color_map_yi[i];
            // combining offsets
            let c_off = cx + cy * COLOR_WIDTH;

            // check if c_off is outside of color image
            // checking rx/cx is not needed because the color image is much wider then the depth image
            if c_off >= COLOR_SIZE {
                depth_to_c_off.push(None);
                continue;
            }

            // saving the offset for later
            depth_to_c_off.push(Some(c_off));

            if enable_filter {
                // setting a window around the filter map pixel corresponding to the color pixel with the current z value
                for y_off in -FILTER_HEIGHT_HALF..FILTER_HEIGHT_HALF {
                    for x_off in -FILTER_WIDTH_HALF..FILTER_WIDTH_HALF {
                        if let (Some(cx), Some(cy)) =
                            (cx.checked_add_signed(x_off), cy.checked_add_signed(y_off))
                        {
                            let offset = cx + cy * COLOR_WIDTH;

                            // only set if the current z is smaller
                            if offset < COLOR_SIZE && z < filter_map[offset] {
                                filter_map[offset] = z;
                            }
                        }
                    }
                }
            }
        }

        /* Construct 'registered' image. */

        // run through all registered color pixels and set them based on filter results if enabled
        for i in 0..DEPTH_SIZE {
            let Some(c_off) = depth_to_c_off[i] else {
                // if offset is out of image
                continue;
            };

            /* Filter drops duplicate pixels due to aspect of two cameras. */
            if enable_filter {
                let min_z = filter_map[c_off];
                let z = undistorted_frame.buffer[i];

                // check for allowed depth noise
                if (z - min_z) / z > FILTER_TOLERANCE {
                    continue;
                }
            }

            let c_off = c_off * bytes_per_pixel;
            let r_off = i * bytes_per_pixel;

            registered_frame.buffer[r_off..r_off + bytes_per_pixel]
                .copy_from_slice(&color_frame.buffer[c_off..c_off + bytes_per_pixel]);
        }

        (registered_frame, undistorted_frame)
    }

    pub fn undistort_depth(&self, depth_frame: &DepthFrame) -> DepthFrame {
        let mut undistorted_frame = DepthFrame {
            width: DEPTH_WIDTH,
            height: DEPTH_HEIGHT,
            buffer: Vec::with_capacity(DEPTH_SIZE),
            sequence: depth_frame.sequence,
            timestamp: depth_frame.timestamp,
        };

        /* Fix depth distortion, and compute pixel to use from 'color' based on depth measurement,
         * stored as x/y offset in the color data.
         */

        // iterating over all pixels from undistorted depth and registered color image
        // the four maps have the same structure as the images, so their pointers are increased each iteration as well
        for i in 0..DEPTH_SIZE {
            // get depth value for current pixel
            undistorted_frame
                .buffer
                .push(depth_frame.buffer[self.distort_map[i]]);
        }

        undistorted_frame
    }

    pub fn xyz_to_point(&self, dx: usize, dy: usize, dz: f32) -> (f32, f32) {
        let index = dx + dy * DEPTH_WIDTH;

        (
            (self.depth_to_color_map_x[index] + (self.color_params.shift_m / dz))
                * self.color_params.fx
                + self.color_params.cx,
            self.depth_to_color_map_y[index],
        )
    }

    pub fn point_to_xyz_pixel(
        &self,
        undistorted_frame: &DepthFrame,
        registered_frame: &ColorFrame,
        x: usize,
        y: usize,
    ) -> (f32, f32, f32, Vec<u8>) {
        let bytes_per_pixel = registered_frame.color_space.bytes_per_pixel();
        let (x, y, z) = self.point_to_xyz(undistorted_frame, x, y);
        let c_off = DEPTH_WIDTH * y as usize + x as usize;
        let pixel = if z.is_nan() {
            vec![0; bytes_per_pixel]
        } else {
            registered_frame.buffer[c_off..c_off + bytes_per_pixel].to_vec()
        };

        (x, y, z, pixel)
    }

    pub fn point_to_xyz(
        &self,
        undistorted_frame: &DepthFrame,
        x: usize,
        y: usize,
    ) -> (f32, f32, f32) {
        let depth_val = undistorted_frame.buffer[DEPTH_WIDTH * y + x] / 1000.0; // scaling factor, so that value of 1 is one meter.

        if depth_val.is_nan() || depth_val <= 0.001 {
            // depth value is not valid
            (NAN, NAN, NAN)
        } else {
            (
                (x as f32 + 0.5 - self.ir_params.cx) * (1.0 / self.ir_params.fx) * depth_val,
                (y as f32 + 0.5 - self.ir_params.cy) * (1.0 / self.ir_params.fy) * depth_val,
                depth_val,
            )
        }
    }

    pub fn distort(&self, mx: usize, my: usize) -> (f32, f32) {
        // see http://en.wikipedia.org/wiki/Distortion_(optics) for description
        let dx = (mx as f32 - self.ir_params.cx) / self.ir_params.fx;
        let dy = (my as f32 - self.ir_params.cy) / self.ir_params.fy;
        let dx2 = dx * dx;
        let dy2 = dy * dy;
        let r2 = dx2 + dy2;
        let dxdy2 = 2.0 * dx * dy;
        let kr = 1.0 + ((self.ir_params.k3 * r2 + self.ir_params.k2) * r2 + self.ir_params.k1) * r2;

        (
            self.ir_params.fx
                * (dx * kr + self.ir_params.p2 * (r2 + 2.0 * dx2) + self.ir_params.p1 * dxdy2)
                + self.ir_params.cx,
            self.ir_params.fy
                * (dy * kr + self.ir_params.p1 * (r2 + 2.0 * dy2) + self.ir_params.p2 * dxdy2)
                + self.ir_params.cy,
        )
    }

    pub fn depth_to_color(&self, mx: f32, my: f32) -> (f32, f32) {
        let mx = (mx - self.ir_params.cx) * DEPTH_Q;
        let my = (my - self.ir_params.cy) * DEPTH_Q;

        let wx = (mx * mx * mx * self.color_params.mx_x3y0)
            + (my * my * my * self.color_params.mx_x0y3)
            + (mx * mx * my * self.color_params.mx_x2y1)
            + (my * my * mx * self.color_params.mx_x1y2)
            + (mx * mx * self.color_params.mx_x2y0)
            + (my * my * self.color_params.mx_x0y2)
            + (mx * my * self.color_params.mx_x1y1)
            + (mx * self.color_params.mx_x1y0)
            + (my * self.color_params.mx_x0y1)
            + (self.color_params.mx_x0y0);

        let wy = (mx * mx * mx * self.color_params.my_x3y0)
            + (my * my * my * self.color_params.my_x0y3)
            + (mx * mx * my * self.color_params.my_x2y1)
            + (my * my * mx * self.color_params.my_x1y2)
            + (mx * mx * self.color_params.my_x2y0)
            + (my * my * self.color_params.my_x0y2)
            + (mx * my * self.color_params.my_x1y1)
            + (mx * self.color_params.my_x1y0)
            + (my * self.color_params.my_x0y1)
            + (self.color_params.my_x0y0);

        (
            (wx / (self.color_params.fx * COLOR_Q))
                - (self.color_params.shift_m / self.color_params.shift_d),
            (wy / COLOR_Q) + self.color_params.cy,
        )
    }
}

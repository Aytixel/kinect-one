use std::{error::Error, f32::consts::PI};

use crate::{
    config::Config,
    data::{DepthProcessorParams, P0Tables},
    processor::ProcessorTrait,
    LUT_SIZE, TABLE_HEIGHT, TABLE_SIZE, TABLE_WIDTH,
};

use super::{DepthFrame, DepthPacket, DepthProcessorTrait, IrFrame};

#[derive(Clone)]
struct Mat<T: Clone + Copy> {
    buffer: Vec<T>,
    width: usize,
}

impl<T: Clone + Copy> Mat<T> {
    fn new<U: Default + Clone + Copy>(width: usize, height: usize) -> Mat<U> {
        Mat {
            buffer: vec![Default::default(); width * height],
            width,
        }
    }

    fn from<U: Into<Vec<T>>>(width: usize, buffer: U) -> Mat<T> {
        Mat {
            buffer: buffer.into(),
            width,
        }
    }

    fn get(&self, x: usize, y: usize) -> T {
        self.buffer[x + y * self.width]
    }

    fn get_mut(&mut self, x: usize, y: usize) -> &mut T {
        &mut self.buffer[x + y * self.width]
    }

    fn copy_from_slice(&mut self, src: &[T]) {
        for (index, value) in src.iter().copied().enumerate() {
            self.buffer[index] = value;
        }
    }

    fn horizontal_flip(&mut self) {
        self.buffer = self
            .buffer
            .chunks_exact(self.width)
            .rev()
            .flatten()
            .cloned()
            .collect();
    }
}

/// Cpu depth processor
pub struct CpuDepthProcessor {
    params: DepthProcessorParams,

    x_table: Mat<f32>,
    z_table: Mat<f32>,

    lut11_to_16: Box<[i16; LUT_SIZE]>,

    trig_table0: [Vec<f32>; 6],
    trig_table1: [Vec<f32>; 6],
    trig_table2: [Vec<f32>; 6],

    enable_bilateral_filter: bool,
    enable_edge_filter: bool,

    flip_ptables: bool,
}

impl CpuDepthProcessor {
    pub fn new() -> Self {
        let mut processor = Self {
            params: DepthProcessorParams::default(),
            x_table: Mat::<f32>::new(TABLE_WIDTH, TABLE_HEIGHT),
            z_table: Mat::<f32>::new(TABLE_WIDTH, TABLE_HEIGHT),
            lut11_to_16: Box::new([0; LUT_SIZE]),
            trig_table0: [
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
            ],
            trig_table1: [
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
            ],
            trig_table2: [
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
                vec![0.0; TABLE_SIZE],
            ],
            enable_bilateral_filter: true,
            enable_edge_filter: true,
            flip_ptables: true,
        };

        processor.set_config(&Config::default());

        processor
    }

    fn decode_pixel_measurement(&self, data: &[u8], sub: usize, x: usize, y: usize) -> i16 {
        if x < 1 || 510 < x || 423 < y {
            return self.lut11_to_16[0];
        }

        let mut r1zi = (x >> 2) + ((x & 0x3) << 7) * 11; // Range 11..5610

        // 298496 = 512 * 424 * 11 / 8 = number of bytes per sub image
        let ptr: &[u16] = unsafe { std::mem::transmute(&data[298496 * sub..]) };
        let i = if y < 212 { y + 212 } else { 423 - y };
        let ptr = &ptr[352 * i..];

        let r1yi = r1zi >> 4; // Range 0..350
        r1zi = r1zi & 15;

        let i1 = (ptr[r1yi] as usize) >> r1zi;
        let i2 = (ptr[r1yi + 1] as usize) << (16 - r1zi);

        return self.lut11_to_16[(i1 | i2) & 2047];
    }

    fn fill_trig_table(
        phase_in_rad: &[f32; 3],
        p0_table: &Mat<u16>,
        trig_table: &mut [Vec<f32>; 6],
    ) {
        for y in 0..TABLE_HEIGHT {
            for x in 0..TABLE_WIDTH {
                let offset = y * TABLE_WIDTH + x;
                let p0 = -(p0_table.get(x, y) as f32) * 0.000031 * PI;

                let tmp0 = p0 + phase_in_rad[0];
                let tmp1 = p0 + phase_in_rad[1];
                let tmp2 = p0 + phase_in_rad[2];

                trig_table[0][offset] = tmp0.cos();
                trig_table[1][offset] = tmp1.cos();
                trig_table[2][offset] = tmp2.cos();

                trig_table[3][offset] = (-tmp0).sin();
                trig_table[4][offset] = (-tmp1).sin();
                trig_table[5][offset] = (-tmp2).sin();
            }
        }
    }

    fn process_measurement_triple(
        &self,
        trig_table: &[Vec<f32>; 6],
        ab_multiplier_per_frq: f32,
        x: usize,
        y: usize,
        m: [i32; 3],
        m_out: &mut [f32],
    ) {
        if self.z_table.get(x, y) > 0.0 {
            if m[0] == 32767 || m[1] == 32767 || m[2] == 32767 {
                m_out[0] = 0.0;
                m_out[1] = 0.0;
                m_out[2] = 65535.0;
            } else {
                let offset = y * TABLE_WIDTH + x;

                // formula given in Patent US 8,587,771 B2
                let mut ir_image_a = trig_table[0][offset] * m[0] as f32
                    + trig_table[1][offset] * m[1] as f32
                    + trig_table[2][offset] * m[2] as f32;
                let mut ir_image_b = trig_table[3][offset] * m[0] as f32
                    + trig_table[4][offset] * m[1] as f32
                    + trig_table[5][offset] * m[2] as f32;

                ir_image_a *= ab_multiplier_per_frq;
                ir_image_b *= ab_multiplier_per_frq;

                let ir_amplitude = (ir_image_a * ir_image_a + ir_image_b * ir_image_b).sqrt()
                    * self.params.ab_multiplier;

                m_out[0] = ir_image_a;
                m_out[1] = ir_image_b;
                m_out[2] = ir_amplitude;
            }
        } else {
            m_out[0] = 0.0;
            m_out[1] = 0.0;
            m_out[2] = 0.0;
        }
    }

    fn process_pixel_stage1(&self, x: usize, y: usize, data: &[u8]) -> [f32; 9] {
        let m0_raw = [
            self.decode_pixel_measurement(data, 0, x, y) as i32,
            self.decode_pixel_measurement(data, 1, x, y) as i32,
            self.decode_pixel_measurement(data, 2, x, y) as i32,
        ];
        let m1_raw = [
            self.decode_pixel_measurement(data, 3, x, y) as i32,
            self.decode_pixel_measurement(data, 4, x, y) as i32,
            self.decode_pixel_measurement(data, 5, x, y) as i32,
        ];
        let m2_raw = [
            self.decode_pixel_measurement(data, 6, x, y) as i32,
            self.decode_pixel_measurement(data, 7, x, y) as i32,
            self.decode_pixel_measurement(data, 8, x, y) as i32,
        ];

        let mut m_out = [0.0; 9];

        self.process_measurement_triple(
            &self.trig_table0,
            self.params.ab_multiplier_per_frq[0],
            x,
            y,
            m0_raw,
            &mut m_out[0..3],
        );
        self.process_measurement_triple(
            &self.trig_table1,
            self.params.ab_multiplier_per_frq[1],
            x,
            y,
            m1_raw,
            &mut m_out[3..6],
        );
        self.process_measurement_triple(
            &self.trig_table2,
            self.params.ab_multiplier_per_frq[2],
            x,
            y,
            m2_raw,
            &mut m_out[6..9],
        );

        m_out
    }

    fn filter_pixel_stage1(&self, x: usize, y: usize, m: &Mat<[f32; 9]>) -> ([f32; 9], bool) {
        let m_ptr = m.get(x, y);
        let mut m_out = [0.0; 9];
        let mut bilateral_max_edge_test = true;

        if x < 1 || y < 1 || x > 510 || y > 422 {
            m_out.copy_from_slice(&m_ptr);
        } else {
            let mut m_normalized = [0.0; 2];
            let mut other_m_normalized = [0.0; 2];

            let mut offset = 0;

            for _ in 0..3 {
                let norm2 = m_ptr[offset] * m_ptr[offset] + m_ptr[offset + 1] * m_ptr[offset + 1];
                let mut inv_norm = 1.0 / norm2.sqrt();
                if inv_norm.is_nan() {
                    inv_norm = f32::INFINITY;
                }

                m_normalized[0] = m_ptr[offset] * inv_norm;
                m_normalized[1] = m_ptr[offset + 1] * inv_norm;

                let mut weight_acc = 0.0;
                let mut weighted_m_acc = [0.0; 2];

                let mut threshold = (self.params.joint_bilateral_ab_threshold
                    * self.params.joint_bilateral_ab_threshold)
                    / (self.params.ab_multiplier * self.params.ab_multiplier);
                let mut joint_bilateral_exp = self.params.joint_bilateral_exp;

                if norm2 < threshold {
                    threshold = 0.0;
                    joint_bilateral_exp = 0.0;
                }

                let mut dist_acc = 0.0;
                let mut j = 0;

                for yi in -1..=1 {
                    for xi in -1..=1 {
                        if yi == 0 && xi == 0 {
                            weight_acc += self.params.gaussian_kernel[j];

                            weighted_m_acc[0] += self.params.gaussian_kernel[j] * m_ptr[offset];
                            weighted_m_acc[1] += self.params.gaussian_kernel[j] * m_ptr[offset + 1];
                            continue;
                        }

                        let other_m_ptr =
                            m.get((x as isize + xi) as usize, (y as isize + yi) as usize);
                        let other_norm2 = other_m_ptr[offset] * other_m_ptr[offset]
                            + other_m_ptr[offset + 1] * other_m_ptr[offset + 1];
                        let mut other_inv_norm = 1.0 / other_norm2.sqrt();
                        if other_inv_norm.is_nan() {
                            other_inv_norm = f32::INFINITY;
                        }

                        other_m_normalized[0] = other_m_ptr[offset] * other_inv_norm;
                        other_m_normalized[1] = other_m_ptr[offset + 1] * other_inv_norm;

                        let dist = (-(other_m_normalized[0] * m_normalized[0]
                            + other_m_normalized[1] * m_normalized[1])
                            + 1.0)
                            * 0.5;

                        let mut weight = 0.0;

                        if other_norm2 >= threshold {
                            weight = self.params.gaussian_kernel[j]
                                * (-1.442695 * joint_bilateral_exp * dist).exp();
                            dist_acc += dist;
                        }

                        weighted_m_acc[0] += weight * other_m_ptr[offset];
                        weighted_m_acc[1] += weight * other_m_ptr[offset + 1];

                        weight_acc += weight;

                        j += 1;
                    }
                }

                bilateral_max_edge_test &= dist_acc < self.params.joint_bilateral_max_edge;

                m_out[offset] = if weight_acc > 0.0 {
                    weighted_m_acc[0] / weight_acc
                } else {
                    0.0
                };
                m_out[offset + 1] = if weight_acc > 0.0 {
                    weighted_m_acc[1] / weight_acc
                } else {
                    0.0
                };
                m_out[offset + 2] = m_ptr[offset + 2];

                offset += 3;
            }
        }

        (m_out, bilateral_max_edge_test)
    }

    fn transform_measurements(&self, m: &mut [f32]) {
        let mut tmp0 = m[1].atan2(m[0]);

        if tmp0 < 0.0 {
            tmp0 += 2.0 * PI;
        }

        m[0] = if tmp0.is_nan() { 0.0 } else { tmp0 }; // phase
        m[1] = (m[0] * m[0] + m[1] * m[1]).sqrt() * self.params.ab_multiplier; // ir amplitude
    }

    fn process_pixel_stage2(&self, x: usize, y: usize, m: &mut [f32; 9]) -> (f32, f32, f32) {
        // Assuming transformMeasurements function is implemented elsewhere
        self.transform_measurements(&mut m[0..3]);
        self.transform_measurements(&mut m[3..6]);
        self.transform_measurements(&mut m[6..9]);

        let m0 = &m[0..3];
        let m1 = &m[3..6];
        let m2 = &m[6..9];

        let ir_sum = m0[1] + m1[1] + m2[1];
        let ir_min = m0[1].min(m1[1]).min(m2[1]);

        let mut phase =
            if ir_min < self.params.individual_ab_threshold || ir_sum < self.params.ab_threshold {
                0.0
            } else {
                let t0 = m0[0] / (2.0 * PI) * 3.0;
                let t1 = m1[0] / (2.0 * PI) * 15.0;
                let t2 = m2[0] / (2.0 * PI) * 2.0;

                let t5 = f32::floor((t1 - t0) * 0.333333 + 0.5) * 3.0 + t0;
                let t3 = -t2 + t5;
                let t4 = t3 * 2.0;

                let mut t3 = t3 * if t4.is_sign_positive() { 0.5 } else { -0.5 };
                t3 = (t3 - f32::floor(t3)) * if t4.is_sign_positive() { 2.0 } else { -2.0 };

                let c2 = 0.5 < t3.abs() && t3.abs() < 1.5;

                let t6 = if c2 { t5 + 15.0 } else { t5 };
                let t7 = if c2 { t1 + 15.0 } else { t1 };

                let mut t8 = (f32::floor((-t2 + t6) * 0.5 + 0.5) * 2.0 + t2) * 0.5;

                let mut t6 = t6 * 0.333333; // = / 3
                let mut t7 = t7 * 0.066667; // = / 15

                let t9 = t8 + t6 + t7; // transformed phase measurements
                let t10 = t9 * 0.333333; // some avg

                t6 *= 2.0 * PI;
                t7 *= 2.0 * PI;
                t8 *= 2.0 * PI;

                // some cross product
                let t8_new = t7 * 0.826977 - t8 * 0.110264;
                let t6_new = t8 * 0.551318 - t6 * 0.826977;
                let t7_new = t6 * 0.110264 - t7 * 0.551318;

                let norm = t8_new * t8_new + t6_new * t6_new + t7_new * t7_new;
                let mask = if t9 >= 0.0 { 1.0 } else { 0.0 };
                let t10 = t10 * mask;

                let mut ir_x = if self.params.ab_confidence_slope > 0.0 {
                    m0[1].min(m1[1]).min(m2[1])
                } else {
                    m0[1].max(m1[1]).max(m2[1])
                };

                ir_x = ir_x.ln();
                ir_x = (ir_x * self.params.ab_confidence_slope * 0.301030
                    + self.params.ab_confidence_offset)
                    * 3.321928;
                ir_x = ir_x.exp();
                ir_x = ir_x
                    .min(self.params.max_dealias_confidence)
                    .max(self.params.min_dealias_confidence);
                ir_x *= ir_x;

                t10 * if ir_x >= norm { 1.0 } else { 0.0 }
            };

        if phase > 0.0 {
            phase += self.params.phase_offset;
        }

        let depth_linear = self.z_table.get(x, y) * phase;
        let max_depth = phase * self.params.unambiguous_dist * 2.0;

        let depth = if depth_linear > 0.0 && max_depth > 0.0 {
            (depth_linear
                / (-depth_linear
                    * ((self.x_table.get(x, y) * 90.0) / (max_depth * max_depth * 8192.0))
                    + 1.0))
                .max(0.0)
        } else {
            depth_linear
        };

        (
            ((m0[2] + m1[2] + m2[2]) * 0.3333333 * self.params.ab_output_multiplier).min(65535.0),
            depth,
            ir_sum,
        )
    }

    fn filter_pixel_stage2(
        &self,
        x: usize,
        y: usize,
        m: &mut Mat<[f32; 3]>, // Assuming m is a 2D vector of Vec<f32, 3>
        max_edge_test_ok: bool,
    ) -> f32 {
        let depth_and_ir_sum = m.get(x, y);
        let raw_depth = depth_and_ir_sum[0];
        let ir_sum = depth_and_ir_sum[2];

        let depth_out = if raw_depth >= self.params.min_depth && raw_depth <= self.params.max_depth
        {
            if x < 1 || y < 1 || x > 510 || y > 422 {
                raw_depth
            } else {
                let mut ir_sum_acc = ir_sum;
                let mut squared_ir_sum_acc = ir_sum * ir_sum;
                let mut min_depth = raw_depth;
                let mut max_depth = raw_depth;

                for yi in -1..=1 {
                    for xi in -1..=1 {
                        if yi == 0 && xi == 0 {
                            continue;
                        }

                        let other = m.get(x.saturating_add_signed(xi), y.saturating_add_signed(yi));

                        ir_sum_acc += other[2];
                        squared_ir_sum_acc += other[2] * other[2];

                        if other[1] > 0.0 {
                            min_depth = min_depth.min(other[1]);
                            max_depth = max_depth.max(other[1]);
                        }
                    }
                }

                let edge_avg = (ir_sum_acc / 9.0).max(self.params.edge_ab_avg_min_value);
                let tmp0 =
                    ((squared_ir_sum_acc * 9.0 - ir_sum_acc * ir_sum_acc).sqrt()) / 9.0 / edge_avg;

                let abs_min_diff = (raw_depth - min_depth).abs();
                let abs_max_diff = (raw_depth - max_depth).abs();

                let avg_diff = (abs_min_diff + abs_max_diff) * 0.5;
                let max_abs_diff = abs_min_diff.max(abs_max_diff);

                let cond0 = raw_depth > 0.0
                    && tmp0 >= self.params.edge_ab_std_dev_threshold
                    && self.params.edge_close_delta_threshold < abs_min_diff
                    && self.params.edge_far_delta_threshold < abs_max_diff
                    && self.params.edge_max_delta_threshold < max_abs_diff
                    && self.params.edge_avg_delta_threshold < avg_diff;

                if cond0 || (max_edge_test_ok && self.params.max_edge_count < 0.0) {
                    0.0
                } else {
                    raw_depth
                }
            }
        } else {
            0.0
        };

        // override raw depth
        m.get_mut(x, y)[0] = depth_and_ir_sum[1];

        depth_out
    }
}

impl DepthProcessorTrait for CpuDepthProcessor {
    fn set_config(&mut self, config: &Config) -> Result<(), Box<dyn Error>> {
        self.params.min_depth = config.min_depth;
        self.params.max_depth = config.max_depth;
        self.enable_bilateral_filter = config.enable_bilateral_filter;
        self.enable_edge_filter = config.enable_edge_aware_filter;

        Ok(())
    }

    fn set_p0_tables(&mut self, p0_tables: &P0Tables) -> Result<(), Box<dyn Error>> {
        let mut p0_table0 = Mat::from(TABLE_WIDTH, p0_tables.p0_table0.to_vec());
        let mut p0_table1 = Mat::from(TABLE_WIDTH, p0_tables.p0_table1.to_vec());
        let mut p0_table2 = Mat::from(TABLE_WIDTH, p0_tables.p0_table2.to_vec());

        if self.flip_ptables {
            p0_table0.horizontal_flip();
            p0_table1.horizontal_flip();
            p0_table2.horizontal_flip();
        }

        Self::fill_trig_table(&self.params.phase_in_rad, &p0_table0, &mut self.trig_table0);
        Self::fill_trig_table(&self.params.phase_in_rad, &p0_table1, &mut self.trig_table1);
        Self::fill_trig_table(&self.params.phase_in_rad, &p0_table2, &mut self.trig_table2);

        Ok(())
    }

    fn set_x_z_tables(
        &mut self,
        x_table: &[f32; TABLE_SIZE],
        z_table: &[f32; TABLE_SIZE],
    ) -> Result<(), Box<dyn Error>> {
        self.x_table.copy_from_slice(x_table);
        self.z_table.copy_from_slice(z_table);

        Ok(())
    }

    fn set_lookup_table(&mut self, lut: &[i16; LUT_SIZE]) -> Result<(), Box<dyn Error>> {
        self.lut11_to_16.copy_from_slice(lut);

        Ok(())
    }
}

impl ProcessorTrait<DepthPacket, (IrFrame, DepthFrame)> for CpuDepthProcessor {
    async fn process(&self, input: DepthPacket) -> Result<(IrFrame, DepthFrame), Box<dyn Error>> {
        let mut ir_frame = IrFrame {
            width: TABLE_WIDTH,
            height: TABLE_HEIGHT,
            buffer: Box::new([0.0; TABLE_SIZE]),
            sequence: input.sequence,
            timestamp: input.timestamp,
        };
        let mut depth_frame = DepthFrame {
            width: TABLE_WIDTH,
            height: TABLE_HEIGHT,
            buffer: Box::new([0.0; TABLE_SIZE]),
            sequence: input.sequence,
            timestamp: input.timestamp,
        };

        let mut m: Mat<[f32; 9]> = Mat::<[f32; 9]>::new(TABLE_WIDTH, TABLE_HEIGHT);
        let mut m_filtered: Mat<[f32; 9]> = Mat::<[f32; 9]>::new(TABLE_WIDTH, TABLE_HEIGHT);
        let mut m_max_edge_test: Mat<bool> = Mat::<bool>::new(TABLE_WIDTH, TABLE_HEIGHT);

        for y in 0..TABLE_HEIGHT {
            for x in 0..TABLE_WIDTH {
                m.get_mut(x, y)
                    .copy_from_slice(&self.process_pixel_stage1(x, y, &input.buffer));
            }
        }

        // bilateral filtering
        let mut m_ptr = if self.enable_bilateral_filter {
            for y in 0..TABLE_HEIGHT {
                for x in 0..TABLE_WIDTH {
                    let (m_filtered_ptr, max_edge_test_val) = self.filter_pixel_stage1(x, y, &m);

                    m_filtered.get_mut(x, y).copy_from_slice(&m_filtered_ptr);
                    *m_max_edge_test.get_mut(x, y) = max_edge_test_val;
                }
            }

            m_filtered
        } else {
            m
        };

        let mut out_ir: Mat<f32> = Mat::<f32>::new(TABLE_WIDTH, TABLE_HEIGHT);
        let mut out_depth: Mat<f32> = Mat::<f32>::new(TABLE_WIDTH, TABLE_HEIGHT);

        if self.enable_edge_filter {
            let mut depth_ir_sum: Mat<[f32; 3]> = Mat::<[f32; 3]>::new(TABLE_WIDTH, TABLE_HEIGHT);

            for y in 0..TABLE_HEIGHT {
                for x in 0..TABLE_WIDTH {
                    let (out_ir_value, raw_depth, ir_sum) =
                        self.process_pixel_stage2(x, y, m_ptr.get_mut(x, y));

                    *out_ir.get_mut(x, 423 - y) = out_ir_value;

                    let depth_ir_sum_ptr = depth_ir_sum.get_mut(x, y);

                    depth_ir_sum_ptr[0] = raw_depth;
                    depth_ir_sum_ptr[1] = if m_max_edge_test.get(x, y) {
                        raw_depth
                    } else {
                        0.0
                    };
                    depth_ir_sum_ptr[2] = ir_sum;
                }
            }

            for y in 0..TABLE_HEIGHT {
                for x in 0..TABLE_WIDTH {
                    *out_depth.get_mut(x, 423 - y) = self.filter_pixel_stage2(
                        x,
                        y,
                        &mut depth_ir_sum,
                        m_max_edge_test.get(x, y),
                    );
                }
            }
        } else {
            for y in 0..TABLE_HEIGHT {
                for x in 0..TABLE_WIDTH {
                    let (out_ir_value, _, out_depth_value) =
                        self.process_pixel_stage2(x, y, m_ptr.get_mut(x, y));

                    *out_ir.get_mut(x, 423 - y) = out_ir_value;
                    *out_depth.get_mut(x, 423 - y) = out_depth_value;
                }
            }
        }

        ir_frame.buffer.copy_from_slice(&out_ir.buffer);
        depth_frame.buffer.copy_from_slice(&out_depth.buffer);

        Ok((ir_frame, depth_frame))
    }
}

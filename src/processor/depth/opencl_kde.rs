use std::{error::Error, f32::consts::PI};

use ocl::{
    builders::BuildOpt,
    prm::{Float, Float3, Float4, Short, Uchar},
    Buffer, Device, Event, Kernel, MemFlags, ProQue, Program,
};

use crate::{
    config::Config, data::P0Tables, processor::ProcessorTrait, settings::DepthProcessorParams,
    DEPTH_HEIGHT, DEPTH_SIZE, DEPTH_WIDTH, LUT_SIZE,
};

use super::{DepthFrame, DepthPacket, DepthProcessorTrait, IrFrame};

macro_rules! build_options {
    (f32 $program_builder:expr => [$($ident:ident = $value:expr $(,)?)*]) => {
        $(
            $program_builder.bo(BuildOpt::IncludeDefine {
                ident: stringify!($ident).to_string(),
                val: format!("{:.16e}f", $value),
            });
        )*
    };
    ($program_builder:expr => [$($ident:ident = $value:expr $(,)?)*]) => {
        $(
            $program_builder.bo(BuildOpt::IncludeDefine {
                ident: stringify!($ident).to_string(),
                val: $value.to_string(),
            });
        )*
    };
}

struct Buffers {
    // Read only
    lut11to16: Buffer<Short>,
    p0_table: Buffer<Float3>,
    x_table: Buffer<f32>,
    z_table: Buffer<f32>,
    packet: Buffer<u16>,
    // Read-Write
    a: Buffer<Float3>,
    b: Buffer<Float3>,
    n: Buffer<Float3>,
    ir: Buffer<f32>,
    a_filtered: Buffer<Float3>,
    b_filtered: Buffer<Float3>,
    edge_test: Buffer<Uchar>,
    depth: Buffer<f32>,
    conf_1: Buffer<Float>,
    conf_2: Buffer<Float>,
    conf_3: Buffer<Float>,
    phase_1: Buffer<Float>,
    phase_2: Buffer<Float>,
    phase_3: Buffer<Float>,
    gaussian_kernel: Buffer<f32>,
    phase_conf: Buffer<Float4>,
}

struct Kernels {
    process_pixel_stage1_kernel: Kernel,
    filter_pixel_stage1_kernel: Kernel,
    process_pixel_stage2_kernel: Kernel,
    filter_pixel_stage2_kernel: Kernel,
}

/// OpenCL Kde depth processor
pub struct OpenCLKdeDepthProcessor {
    device: Device,
    params: DepthProcessorParams,
    config: Config,
    buffers: Buffers,
    kernels: Kernels,
}

impl OpenCLKdeDepthProcessor {
    pub fn new(device: Device) -> Result<Self, Box<dyn Error>> {
        let params = DepthProcessorParams::default();
        let config = Config::default();

        let (buffers, kernels) = Self::create_program(&params, &config, &device)?;

        Ok(Self {
            device,
            params,
            config,
            buffers,
            kernels,
        })
    }

    fn create_program(
        params: &DepthProcessorParams,
        config: &Config,
        device: &Device,
    ) -> Result<(Buffers, Kernels), Box<dyn Error>> {
        let mut program_builder = Program::builder();

        program_builder
            .src(include_str!(
                "./opencl/opencl_kde_depth_packet_processor.cl"
            ))
            .cmplr_opt("-cl-mad-enable")
            .cmplr_opt("-cl-no-signed-zeros")
            .cmplr_opt("-cl-fast-relaxed-math");

        build_options!(
            program_builder => [
                BFI_BITMASK = 0x180,

                KDE_NEIGBORHOOD_SIZE = params.kde_neigborhood_size,
            ]
        );

        build_options!(
            f32 program_builder => [
                AB_MULTIPLIER = params.ab_multiplier,
                AB_MULTIPLIER_PER_FRQ0 = params.ab_multiplier_per_frq[0],
                AB_MULTIPLIER_PER_FRQ1 = params.ab_multiplier_per_frq[1],
                AB_MULTIPLIER_PER_FRQ2 = params.ab_multiplier_per_frq[2],
                AB_OUTPUT_MULTIPLIER = params.ab_output_multiplier,

                PHASE_IN_RAD0 = params.phase_in_rad[0],
                PHASE_IN_RAD1 = params.phase_in_rad[1],
                PHASE_IN_RAD2 = params.phase_in_rad[2],

                JOINT_BILATERAL_AB_THRESHOLD = params.joint_bilateral_ab_threshold,
                JOINT_BILATERAL_MAX_EDGE = params.joint_bilateral_max_edge,
                JOINT_BILATERAL_EXP = params.joint_bilateral_exp,
                JOINT_BILATERAL_THRESHOLD = (params.joint_bilateral_ab_threshold * params.joint_bilateral_ab_threshold) / (params.ab_multiplier * params.ab_multiplier),
                GAUSSIAN_KERNEL_0 = params.gaussian_kernel[0],
                GAUSSIAN_KERNEL_1 = params.gaussian_kernel[1],
                GAUSSIAN_KERNEL_2 = params.gaussian_kernel[2],
                GAUSSIAN_KERNEL_3 = params.gaussian_kernel[3],
                GAUSSIAN_KERNEL_4 = params.gaussian_kernel[4],
                GAUSSIAN_KERNEL_5 = params.gaussian_kernel[5],
                GAUSSIAN_KERNEL_6 = params.gaussian_kernel[6],
                GAUSSIAN_KERNEL_7 = params.gaussian_kernel[7],
                GAUSSIAN_KERNEL_8 = params.gaussian_kernel[8],

                PHASE_OFFSET = params.phase_offset,
                UNAMBIGUOUS_DIST = params.unambiguous_dist,
                INDIVIDUAL_AB_THRESHOLD = params.individual_ab_threshold,
                AB_THRESHOLD = params.ab_threshold,
                AB_CONFIDENCE_SLOPE = params.ab_confidence_slope,
                AB_CONFIDENCE_OFFSET = params.ab_confidence_offset,
                MIN_DEALIAS_CONFIDENCE = params.min_dealias_confidence,
                MAX_DEALIAS_CONFIDENCE = params.max_dealias_confidence,

                EDGE_AB_AVG_MIN_VALUE = params.edge_ab_avg_min_value,
                EDGE_AB_STD_DEV_THRESHOLD = params.edge_ab_std_dev_threshold,
                EDGE_CLOSE_DELTA_THRESHOLD = params.edge_close_delta_threshold,
                EDGE_FAR_DELTA_THRESHOLD = params.edge_far_delta_threshold,
                EDGE_MAX_DELTA_THRESHOLD = params.edge_max_delta_threshold,
                EDGE_AVG_DELTA_THRESHOLD = params.edge_avg_delta_threshold,
                MAX_EDGE_COUNT = params.max_edge_count,

                MIN_DEPTH = config.min_depth * 1000.0,
                MAX_DEPTH = config.max_depth * 1000.0,

                KDE_SIGMA_SQR = params.kde_sigma_sqr,
                UNWRAPPING_LIKELIHOOD_SCALE = params.unwrapping_likelihood_scale,
                PHASE_CONFIDENCE_SCALE = params.phase_confidence_scale,
                KDE_THRESHOLD = params.kde_threshold,
            ]
        );

        let pro_que = ProQue::builder()
            .dims(DEPTH_SIZE)
            .prog_bldr(program_builder)
            .device(device)
            .build()?;

        let buffers = Buffers {
            lut11to16: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_ONLY)
                .len(LUT_SIZE)
                .build()?,
            p0_table: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_ONLY)
                .len(DEPTH_SIZE)
                .build()?,
            x_table: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_ONLY)
                .len(DEPTH_SIZE)
                .build()?,
            z_table: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_ONLY)
                .len(DEPTH_SIZE)
                .build()?,
            packet: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_ONLY)
                .len(((DEPTH_SIZE * 11) / 16) * 10)
                .build()?,
            a: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            b: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            n: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            ir: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            a_filtered: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            b_filtered: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            edge_test: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            depth: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            conf_1: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            conf_2: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            conf_3: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            phase_1: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            phase_2: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            phase_3: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            gaussian_kernel: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
            phase_conf: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(DEPTH_SIZE)
                .build()?,
        };
        let kernels = Kernels {
            process_pixel_stage1_kernel: pro_que
                .kernel_builder("processPixelStage1")
                .arg(&buffers.lut11to16)
                .arg(&buffers.z_table)
                .arg(&buffers.p0_table)
                .arg(&buffers.packet)
                .arg(&buffers.a)
                .arg(&buffers.b)
                .arg(&buffers.n)
                .arg(&buffers.ir)
                .build()?,
            filter_pixel_stage1_kernel: pro_que
                .kernel_builder("filterPixelStage1")
                .arg(&buffers.a)
                .arg(&buffers.b)
                .arg(&buffers.n)
                .arg(&buffers.a_filtered)
                .arg(&buffers.b_filtered)
                .arg(&buffers.edge_test)
                .build()?,
            process_pixel_stage2_kernel: if params.num_hyps == 3 {
                pro_que
                    .kernel_builder("processPixelStage2_phase3")
                    .arg(if config.enable_bilateral_filter {
                        &buffers.a_filtered
                    } else {
                        &buffers.a
                    })
                    .arg(if config.enable_bilateral_filter {
                        &buffers.b_filtered
                    } else {
                        &buffers.b
                    })
                    .arg(&buffers.phase_1)
                    .arg(&buffers.phase_2)
                    .arg(&buffers.phase_3)
                    .arg(&buffers.conf_1)
                    .arg(&buffers.conf_2)
                    .arg(&buffers.conf_3)
                    .build()?
            } else {
                pro_que
                    .kernel_builder("processPixelStage2_phase")
                    .arg(if config.enable_bilateral_filter {
                        &buffers.a_filtered
                    } else {
                        &buffers.a
                    })
                    .arg(if config.enable_bilateral_filter {
                        &buffers.b_filtered
                    } else {
                        &buffers.b
                    })
                    .arg(&buffers.phase_conf)
                    .build()?
            },
            filter_pixel_stage2_kernel: if params.num_hyps == 3 {
                pro_que
                    .kernel_builder("filter_kde3")
                    .arg(&buffers.phase_1)
                    .arg(&buffers.phase_2)
                    .arg(&buffers.phase_3)
                    .arg(&buffers.conf_1)
                    .arg(&buffers.conf_2)
                    .arg(&buffers.conf_3)
                    .arg(&buffers.gaussian_kernel)
                    .arg(&buffers.z_table)
                    .arg(&buffers.x_table)
                    .arg(&buffers.depth)
                    .build()?
            } else {
                pro_que
                    .kernel_builder("filter_kde")
                    .arg(&buffers.phase_conf)
                    .arg(&buffers.gaussian_kernel)
                    .arg(&buffers.z_table)
                    .arg(&buffers.x_table)
                    .arg(&buffers.depth)
                    .build()?
            },
        };

        Ok((buffers, kernels))
    }
}

impl DepthProcessorTrait for OpenCLKdeDepthProcessor {
    fn set_config(&mut self, config: &Config) -> Result<(), Box<dyn Error>> {
        self.config = config.clone();

        let (buffers, kernels) = Self::create_program(&self.params, &config, &self.device)?;

        self.buffers = buffers;
        self.kernels = kernels;

        Ok(())
    }

    fn set_p0_tables(&mut self, p0_tables: &P0Tables) -> Result<(), Box<dyn Error>> {
        let mut p0_table = Vec::with_capacity(DEPTH_SIZE);

        for r in 0..DEPTH_HEIGHT {
            for c in 0..DEPTH_WIDTH {
                p0_table.push(Float3::new(
                    -(p0_tables.p0_table0[r * DEPTH_WIDTH + c] as f32) * 0.000031 * PI,
                    -(p0_tables.p0_table1[r * DEPTH_WIDTH + c] as f32) * 0.000031 * PI,
                    -(p0_tables.p0_table2[r * DEPTH_WIDTH + c] as f32) * 0.000031 * PI,
                ));
            }
        }

        self.buffers.p0_table.write(&p0_table).enq()?;

        Ok(())
    }

    fn set_x_z_tables(
        &mut self,
        x_table: &[f32; DEPTH_SIZE],
        z_table: &[f32; DEPTH_SIZE],
    ) -> Result<(), Box<dyn Error>> {
        self.buffers.x_table.write(x_table.as_slice()).enq()?;
        self.buffers.z_table.write(z_table.as_slice()).enq()?;

        let mut gaussian_kernel = vec![0.0; self.params.kde_neigborhood_size * 2 + 1];
        let sigma = self.params.kde_neigborhood_size as f32 * 0.5;

        for i in
            -(self.params.kde_neigborhood_size as isize)..=self.params.kde_neigborhood_size as isize
        {
            gaussian_kernel[self.params.kde_neigborhood_size.saturating_add_signed(i)] =
                (-0.5 * i as f32 * i as f32 / (sigma * sigma)).exp();
        }

        self.buffers
            .gaussian_kernel
            .write(gaussian_kernel.as_slice())
            .enq()?;

        Ok(())
    }

    fn set_lookup_table(&mut self, lut: &[i16; LUT_SIZE]) -> Result<(), Box<dyn Error>> {
        self.buffers
            .lut11to16
            .write(
                &lut.iter()
                    .map(|value| Short::new(*value))
                    .collect::<Vec<_>>(),
            )
            .enq()?;

        Ok(())
    }
}

impl ProcessorTrait<DepthPacket, (IrFrame, DepthFrame)> for OpenCLKdeDepthProcessor {
    async fn process(&self, input: DepthPacket) -> Result<(IrFrame, DepthFrame), Box<dyn Error>> {
        let mut ir_frame = IrFrame {
            width: DEPTH_WIDTH,
            height: DEPTH_HEIGHT,
            buffer: vec![0.0; DEPTH_SIZE],
            sequence: input.sequence,
            timestamp: input.timestamp,
        };
        let mut depth_frame = DepthFrame {
            width: DEPTH_WIDTH,
            height: DEPTH_HEIGHT,
            buffer: vec![0.0; DEPTH_SIZE],
            sequence: input.sequence,
            timestamp: input.timestamp,
        };

        let mut event_write = Event::empty();
        let mut event_pps1 = Event::empty();
        let mut event_fps1 = Event::empty();
        let mut event_pps2 = Event::empty();
        let mut event_fps2 = Event::empty();
        let mut event_read_ir = Event::empty();
        let mut event_read_depth = Event::empty();

        self.buffers
            .packet
            .write(
                &input
                    .buffer
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect::<Vec<u16>>(),
            )
            .enew(&mut event_write)
            .enq()?;

        unsafe {
            self.kernels
                .process_pixel_stage1_kernel
                .cmd()
                .ewait(&event_write)
                .enew(&mut event_pps1)
                .enq()?;
        }

        self.buffers
            .ir
            .read(ir_frame.buffer.as_mut_slice())
            .ewait(&event_pps1)
            .enew(&mut event_read_ir)
            .enq()?;

        if self.config.enable_bilateral_filter {
            unsafe {
                self.kernels
                    .filter_pixel_stage1_kernel
                    .cmd()
                    .ewait(&event_pps1)
                    .enew(&mut event_fps1)
                    .enq()?;
            }
        } else {
            event_fps1 = event_pps1;
        }

        unsafe {
            self.kernels
                .process_pixel_stage2_kernel
                .cmd()
                .ewait(&event_fps1)
                .enew(&mut event_pps2)
                .enq()?;
        }

        unsafe {
            self.kernels
                .filter_pixel_stage2_kernel
                .cmd()
                .ewait(&event_pps2)
                .enew(&mut event_fps2)
                .enq()?;
        }

        self.buffers
            .depth
            .read(depth_frame.buffer.as_mut_slice())
            .ewait(&event_fps2)
            .enew(&mut event_read_depth)
            .enq()?;

        event_read_ir.wait_for()?;
        event_read_depth.wait_for()?;

        Ok((ir_frame, depth_frame))
    }
}

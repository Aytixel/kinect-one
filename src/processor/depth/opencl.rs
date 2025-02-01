use std::{error::Error, f32::consts::PI};

use ocl::{
    builders::BuildOpt,
    prm::{Float, Float3, Short, Uchar},
    Buffer, Device, Event, Kernel, MemFlags, ProQue, Program,
};

use crate::{
    config::Config, data::P0Tables, processor::ProcessorTrait, settings::DepthProcessorParams,
    LUT_SIZE, TABLE_HEIGHT, TABLE_SIZE, TABLE_WIDTH,
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
    packet: Buffer<u8>,
    // Read-Write
    a: Buffer<Float3>,
    b: Buffer<Float3>,
    n: Buffer<Float3>,
    ir: Buffer<f32>,
    a_filtered: Buffer<Float3>,
    b_filtered: Buffer<Float3>,
    edge_test: Buffer<Uchar>,
    depth: Buffer<f32>,
    ir_sum: Buffer<Float>,
    filtered: Buffer<f32>,
}

struct Kernels {
    process_pixel_stage1_kernel: Kernel,
    filter_pixel_stage1_kernel: Kernel,
    process_pixel_stage2_kernel: Kernel,
    filter_pixel_stage2_kernel: Kernel,
}

/// OpenCL depth processor
pub struct OpenCLDepthProcessor {
    device: Device,
    params: DepthProcessorParams,
    config: Config,
    buffers: Buffers,
    kernels: Kernels,
}

impl OpenCLDepthProcessor {
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
            .src(include_str!("./opencl/opencl_depth_packet_processor.cl"))
            .cmplr_opt("-cl-mad-enable")
            .cmplr_opt("-cl-no-signed-zeros")
            .cmplr_opt("-cl-fast-relaxed-math");

        build_options!(
            program_builder => [
                BFI_BITMASK = 0x180,
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
            ]
        );

        let pro_que = ProQue::builder()
            .dims(TABLE_SIZE)
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
                .len(TABLE_SIZE)
                .build()?,
            x_table: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_ONLY)
                .len(TABLE_SIZE)
                .build()?,
            z_table: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_ONLY)
                .len(TABLE_SIZE)
                .build()?,
            packet: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_ONLY)
                .len(((TABLE_SIZE * 11) / 16) * 10 * size_of::<u16>())
                .build()?,
            a: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(TABLE_SIZE)
                .build()?,
            b: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(TABLE_SIZE)
                .build()?,
            n: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(TABLE_SIZE)
                .build()?,
            ir: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(TABLE_SIZE)
                .build()?,
            a_filtered: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(TABLE_SIZE)
                .build()?,
            b_filtered: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(TABLE_SIZE)
                .build()?,
            edge_test: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(TABLE_SIZE)
                .build()?,
            depth: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(TABLE_SIZE)
                .build()?,
            ir_sum: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(TABLE_SIZE)
                .build()?,
            filtered: pro_que
                .buffer_builder()
                .flags(MemFlags::READ_WRITE)
                .len(TABLE_SIZE)
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
            process_pixel_stage2_kernel: pro_que
                .kernel_builder("processPixelStage2")
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
                .arg(&buffers.x_table)
                .arg(&buffers.z_table)
                .arg(&buffers.depth)
                .arg(&buffers.ir_sum)
                .build()?,
            filter_pixel_stage2_kernel: pro_que
                .kernel_builder("filterPixelStage2")
                .arg(&buffers.depth)
                .arg(&buffers.ir_sum)
                .arg(&buffers.edge_test)
                .arg(&buffers.filtered)
                .build()?,
        };

        Ok((buffers, kernels))
    }
}

impl DepthProcessorTrait for OpenCLDepthProcessor {
    fn set_config(&mut self, config: &Config) -> Result<(), Box<dyn Error>> {
        self.config = config.clone();

        let (buffers, kernels) = Self::create_program(&self.params, &config, &self.device)?;

        self.buffers = buffers;
        self.kernels = kernels;

        Ok(())
    }

    fn set_p0_tables(&mut self, p0_tables: &P0Tables) -> Result<(), Box<dyn Error>> {
        let mut p0_table = Vec::with_capacity(TABLE_SIZE);

        for r in 0..TABLE_HEIGHT {
            for c in 0..TABLE_WIDTH {
                p0_table.push(Float3::new(
                    -(p0_tables.p0_table0[r * TABLE_WIDTH + c] as f32) * 0.000031 * PI,
                    -(p0_tables.p0_table1[r * TABLE_WIDTH + c] as f32) * 0.000031 * PI,
                    -(p0_tables.p0_table2[r * TABLE_WIDTH + c] as f32) * 0.000031 * PI,
                ));
            }
        }

        self.buffers.p0_table.write(&p0_table).enq()?;

        Ok(())
    }

    fn set_x_z_tables(
        &mut self,
        x_table: &[f32; TABLE_SIZE],
        z_table: &[f32; TABLE_SIZE],
    ) -> Result<(), Box<dyn Error>> {
        self.buffers.x_table.write(x_table.as_slice()).enq()?;
        self.buffers.z_table.write(z_table.as_slice()).enq()?;

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

impl ProcessorTrait<DepthPacket, (IrFrame, DepthFrame)> for OpenCLDepthProcessor {
    async fn process(&self, input: DepthPacket) -> Result<(IrFrame, DepthFrame), Box<dyn Error>> {
        let mut ir_frame = IrFrame {
            width: TABLE_WIDTH,
            height: TABLE_HEIGHT,
            buffer: vec![0.0; TABLE_SIZE],
            sequence: input.sequence,
            timestamp: input.timestamp,
        };
        let mut depth_frame = DepthFrame {
            width: TABLE_WIDTH,
            height: TABLE_HEIGHT,
            buffer: vec![0.0; TABLE_SIZE],
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
            .write(&input.buffer)
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

        if self.config.enable_edge_aware_filter {
            unsafe {
                self.kernels
                    .filter_pixel_stage2_kernel
                    .cmd()
                    .ewait(&event_pps2)
                    .enew(&mut event_fps2)
                    .enq()?;
            }
        } else {
            event_fps2 = event_pps2;
        }

        if self.config.enable_edge_aware_filter {
            self.buffers
                .filtered
                .read(depth_frame.buffer.as_mut_slice())
                .ewait(&event_fps2)
                .enew(&mut event_read_depth)
                .enq()?;
        } else {
            self.buffers
                .depth
                .read(depth_frame.buffer.as_mut_slice())
                .ewait(&event_fps2)
                .enew(&mut event_read_depth)
                .enq()?;
        }

        event_read_ir.wait_for()?;
        event_read_depth.wait_for()?;

        Ok((ir_frame, depth_frame))
    }
}

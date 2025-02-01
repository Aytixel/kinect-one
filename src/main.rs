use std::{error::Error, fs::write};

use kinect_one::{
    processor::{
        color::{ColorSpace, MozColorProcessor},
        depth::{DepthProcessorTrait, OpenCLKdeDepthProcessor},
        ProcessTrait, Registration,
    },
    DeviceEnumerator, DEPTH_HEIGHT, DEPTH_SIZE, DEPTH_WIDTH,
};
use mozjpeg::Compress;
use ocl::{Device, Platform};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut devices = DeviceEnumerator::new()?;
    let mut device = devices.open_default(true)?;

    println!("Starting");
    device.start()?;
    println!("Started");

    let mut registration = Registration::new();

    registration.set_ir_params(device.get_ir_params());
    registration.set_color_params(device.get_color_params());

    let color_processor = MozColorProcessor::new(ColorSpace::RGB, false, false);
    let mut depth_processor = OpenCLKdeDepthProcessor::new(Device::first(Platform::first()?)?)?;

    depth_processor.set_p0_tables(device.get_p0_tables())?;
    depth_processor.set_ir_params(device.get_ir_params())?;

    let mut color_frame = None;
    let mut depth_frame = None;

    loop {
        if let Ok(Some(frame)) = device.poll_color_frame() {
            color_frame = Some(frame.process(&color_processor).await?);
        }
        if let Ok(Some(frame)) = device.poll_depth_frame() {
            depth_frame = Some(frame.process(&depth_processor).await?.1);
        }
        if let (Some(color_frame), Some(depth_frame)) = (&color_frame, &depth_frame) {
            let (registered_frame, undistorted_frame) =
                registration.undistort_depth_and_color(color_frame, depth_frame, false);

            let mut comp = Compress::new(mozjpeg::ColorSpace::JCS_RGB);

            comp.set_size(depth_frame.width, depth_frame.height);

            let mut comp = comp.start_compress(Vec::new())?;

            let mut buffer = Vec::with_capacity(DEPTH_SIZE * 3);

            for y in 0..DEPTH_HEIGHT {
                for x in 0..DEPTH_WIDTH {
                    buffer.extend(
                        registration
                            .point_to_xyz_pixel(&undistorted_frame, &registered_frame, x, y)
                            .3,
                    );
                }
            }

            comp.write_scanlines(&registered_frame.buffer)?;

            write("t.jpeg", comp.finish()?)?;
        }
    }

    device.close()?;

    Ok(())
}

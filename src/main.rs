use std::{error::Error, fs::write};

use kinect_one::{
    processor::{
        depth::{CpuDepthProcessor, DepthProcessorTrait, OpenCLDepthProcessor},
        rgb::{ColorSpace, MozRgbProcessor},
        ProcessTrait,
    },
    DeviceEnumerator,
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

    let rgb_processor = MozRgbProcessor::new(ColorSpace::YCbCr, false, false);
    let mut depth_processor = OpenCLDepthProcessor::new(Device::first(Platform::first()?)?)?;

    depth_processor.set_p0_tables(device.get_p0_tables())?;
    depth_processor.set_ir_params(device.get_ir_params())?;

    loop {
        if let Ok(Some(frame)) = device.poll_rgb_frame() {
            println!("rgb: {:?}", frame);
            println!("rgb: {:?}", frame.process(&rgb_processor).await);
        }
        if let Ok(Some(frame)) = device.poll_depth_frame() {
            println!("depth: {:?}", frame);

            let frame = frame.process(&depth_processor).await?;

            let mut comp = Compress::new(mozjpeg::ColorSpace::JCS_RGB);

            comp.set_size(frame.0.width, frame.0.height);

            let mut comp = comp.start_compress(Vec::new())?;

            let data = frame
                .0
                .buffer
                .iter()
                .flat_map(|value| {
                    let value = (value % 256.0) as u8;

                    [value, value, value]
                })
                .collect::<Vec<_>>();

            comp.write_scanlines(&data)?;

            write("t.jpeg", comp.finish()?)?;
        }
    }

    device.close()?;

    Ok(())
}

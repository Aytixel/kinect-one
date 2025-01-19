use std::error::Error;

use kinect_one::{
    processor::{
        depth::{CpuDepthProcessor, DepthProcessorTrait},
        rgb::{ColorSpace, MozRgbProcessor},
        ProcessTrait,
    },
    DeviceEnumerator,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut devices = DeviceEnumerator::new()?;
    let mut device = devices.open_default(true)?;

    println!("Starting");
    device.start()?;
    println!("Started");

    let rgb_processor = MozRgbProcessor::new(ColorSpace::YCbCr, false, false);
    let mut depth_processor = CpuDepthProcessor::new();

    depth_processor.set_p0_tables(device.get_p0_tables());
    depth_processor.set_ir_params(device.get_ir_params());

    loop {
        if let Ok(Some(frame)) = device.poll_rgb_frame() {
            println!("rgb: {:?}", frame);
            println!("rgb: {:?}", frame.process(&rgb_processor).await);
        }
        if let Ok(Some(frame)) = device.poll_depth_frame() {
            println!("depth: {:?}", frame);
            println!("depth: {:?}", frame.process(&depth_processor).await);
            // frame.process(&depth_processor).await;
            return Ok(());
        }
    }

    device.close()?;

    Ok(())
}

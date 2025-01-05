use std::error::Error;

use kinect_one::{
    processor::{
        rgb::{ColorSpace, MozRgbProcessor},
        NoopProcessor, ProcessTrait,
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

    loop {
        if let Ok(Some(frame)) = device.poll_rgb_frame() {
            println!("rgb: {:?}", frame);
            println!(
                "rgb: {:?}",
                frame
                    .process(&MozRgbProcessor::new(ColorSpace::YCbCr, false, false))
                    .await
            );
        }
        if let Ok(Some(frame)) = device.poll_depth_frame() {
            println!("depth: {:?}", frame);
            println!("depth: {:?}", frame.process(&NoopProcessor).await);
        }
    }

    device.close()?;

    Ok(())
}

use std::error::Error;

use two_kinect::{
    processor::rgb::{ColorSpace, MozRgbProcessor},
    Device,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut device = Device::open_default(true).await?;

    println!("{:?}", device.get_firware_versions().await?[3]);

    device.start().await?;

    println!(
        "{:?}",
        device
            .process_rgb_frame(&MozRgbProcessor::new(ColorSpace::YCbCr, false, false))
            .await?
    );

    device.close().await?;

    Ok(())
}

use two_kinect::{Device, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let mut device = Device::open_default(true).await?;

    device.start().await?;

    device.close().await?;

    Ok(())
}

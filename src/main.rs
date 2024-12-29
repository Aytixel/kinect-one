use std::time::Duration;

use two_kinect::{
    config::{LedId, LedSettings},
    Device, Error,
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut device = Device::open_default().await?;

    device
        .set_led_status(LedSettings::blink(
            LedId::Primary,
            10,
            100,
            Duration::from_secs(1),
        ))
        .await?;

    Ok(())
}

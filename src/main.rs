use two_kinect::{Device, Error};

fn main() -> Result<(), Error> {
    if let Some(device) = Device::enumerate_device()?.next() {
        let device = device.open()?;
    }

    Ok(())
}

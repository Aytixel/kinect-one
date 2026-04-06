use std::error::Error;

use fev::{
    display::Display,
    image::{Image, ImageFormat},
    jpeg::JpegDecodeSession,
    PixelFormat,
};
use winit::event_loop::EventLoop;

use crate::{processor::ProcessorTrait, COLOR_HEIGHT, COLOR_WIDTH};

use super::{ColorFrame, ColorPacket, ColorSpace};

impl From<PixelFormat> for ColorSpace {
    fn from(value: PixelFormat) -> Self {
        match value {
            PixelFormat::RGBA => Self::RGBA,
            PixelFormat::RGBX => Self::RGBX,
            PixelFormat::YUYV => Self::YCbCr,
            PixelFormat::BGRA => Self::BGRA,
            PixelFormat::BGRX => Self::BGRX,
            _ => Self::Unknown,
        }
    }
}

impl TryInto<PixelFormat> for ColorSpace {
    type Error = &'static str;

    fn try_into(self) -> Result<PixelFormat, Self::Error> {
        match self {
            ColorSpace::RGB => Err("RGB is not supported by FeV"),
            ColorSpace::RGBA => Ok(PixelFormat::RGBA),
            ColorSpace::RGBX => Ok(PixelFormat::RGBX),
            ColorSpace::YCbCr => Ok(PixelFormat::YUYV),
            ColorSpace::BGR => Err("BGR is not supported by FeV"),
            ColorSpace::BGRA => Ok(PixelFormat::BGRA),
            ColorSpace::BGRX => Ok(PixelFormat::BGRX),
            ColorSpace::Unknown => Err("Unknown is not supported by FeV"),
        }
    }
}

/// FeV (LibVA) color processor
pub struct FeVColorProcessor {
    color_space: PixelFormat,
    display: Display,
}

impl FeVColorProcessor {
    pub fn new(color_space: ColorSpace) -> Result<Self, Box<dyn Error>> {
        let display = Display::new(EventLoop::new()?.owned_display_handle())?;

        Ok(Self {
            color_space: color_space.try_into()?,
            display,
        })
    }
}

impl ProcessorTrait<ColorPacket, ColorFrame> for FeVColorProcessor {
    async fn process(&self, input: ColorPacket) -> Result<ColorFrame, Box<dyn Error>> {
        let mut jpeg_decode_session =
            JpegDecodeSession::new(&self.display, COLOR_WIDTH as u16, COLOR_HEIGHT as u16)?;
        let mut image = Image::new(
            &self.display,
            ImageFormat::new(self.color_space),
            COLOR_WIDTH as u32,
            COLOR_HEIGHT as u32,
        )?;

        jpeg_decode_session
            .decode(&input.jpeg_buffer)?
            .copy_to_image(&mut image)?;

        let mapping = image.map()?;

        Ok(ColorFrame::from_packet(
            self.color_space.into(),
            mapping.to_vec(),
            &input,
        ))
    }
}

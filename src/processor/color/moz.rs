use std::error::Error;

use mozjpeg::Decompress;

use crate::processor::ProcessorTrait;

use super::{ColorFrame, ColorSpace, ColorPacket};

impl From<mozjpeg::ColorSpace> for ColorSpace {
    fn from(value: mozjpeg::ColorSpace) -> Self {
        match value {
            mozjpeg::ColorSpace::JCS_RGB => Self::RGB,
            mozjpeg::ColorSpace::JCS_YCbCr => Self::YCbCr,
            mozjpeg::ColorSpace::JCS_EXT_RGB => Self::BGR,
            mozjpeg::ColorSpace::JCS_EXT_RGBX | mozjpeg::ColorSpace::JCS_EXT_RGBA => Self::RGBA,
            mozjpeg::ColorSpace::JCS_EXT_BGR => Self::BGR,
            mozjpeg::ColorSpace::JCS_EXT_BGRX | mozjpeg::ColorSpace::JCS_EXT_BGRA => Self::BGRA,
            _ => Self::Unknown,
        }
    }
}

impl Into<mozjpeg::ColorSpace> for ColorSpace {
    fn into(self) -> mozjpeg::ColorSpace {
        match self {
            ColorSpace::RGB => mozjpeg::ColorSpace::JCS_EXT_RGB,
            ColorSpace::RGBA => mozjpeg::ColorSpace::JCS_EXT_RGBA,
            ColorSpace::YCbCr => mozjpeg::ColorSpace::JCS_YCbCr,
            ColorSpace::BGR => mozjpeg::ColorSpace::JCS_EXT_BGR,
            ColorSpace::BGRA => mozjpeg::ColorSpace::JCS_EXT_BGRA,
            ColorSpace::Unknown => mozjpeg::ColorSpace::JCS_UNKNOWN,
        }
    }
}

/// MozJpeg color processor
pub struct MozColorProcessor {
    colorspace: mozjpeg::ColorSpace,
    fancy_upsampling: bool,
    block_smoothing: bool,
}

impl MozColorProcessor {
    pub fn new(colorspace: ColorSpace, fancy_upsampling: bool, block_smoothing: bool) -> Self {
        Self {
            colorspace: colorspace.into(),
            fancy_upsampling,
            block_smoothing,
        }
    }
}

impl ProcessorTrait<ColorPacket, ColorFrame> for MozColorProcessor {
    async fn process(&self, input: ColorPacket) -> Result<ColorFrame, Box<dyn Error>> {
        let mut decoder = Decompress::new_mem(&input.jpeg_buffer)?;

        decoder.do_fancy_upsampling(self.fancy_upsampling);
        decoder.do_block_smoothing(self.block_smoothing);

        let mut decoder = decoder.to_colorspace(self.colorspace)?;
        let buffer = decoder.read_scanlines()?;

        Ok(ColorFrame {
            color_space: decoder.color_space().into(),
            width: decoder.width(),
            height: decoder.height(),
            buffer,
            sequence: input.sequence,
            timestamp: input.timestamp,
            exposure: input.exposure,
            gain: input.gain,
            gamma: input.gamma,
        })
    }
}

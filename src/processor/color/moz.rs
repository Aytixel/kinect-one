use std::error::Error;

use mozjpeg::{DctMethod, Decompress};

use crate::processor::ProcessorTrait;

use super::{ColorFrame, ColorPacket, ColorSpace};

impl From<mozjpeg::ColorSpace> for ColorSpace {
    fn from(value: mozjpeg::ColorSpace) -> Self {
        match value {
            mozjpeg::ColorSpace::JCS_RGB => Self::RGB,
            mozjpeg::ColorSpace::JCS_YCbCr => Self::YCbCr,
            mozjpeg::ColorSpace::JCS_EXT_RGB => Self::BGR,
            mozjpeg::ColorSpace::JCS_EXT_RGBA => Self::RGBA,
            mozjpeg::ColorSpace::JCS_EXT_RGBX => Self::RGBX,
            mozjpeg::ColorSpace::JCS_EXT_BGR => Self::BGR,
            mozjpeg::ColorSpace::JCS_EXT_BGRA => Self::BGRA,
            mozjpeg::ColorSpace::JCS_EXT_BGRX => Self::BGRX,
            _ => Self::Unknown,
        }
    }
}

impl Into<mozjpeg::ColorSpace> for ColorSpace {
    fn into(self) -> mozjpeg::ColorSpace {
        match self {
            ColorSpace::RGB => mozjpeg::ColorSpace::JCS_EXT_RGB,
            ColorSpace::RGBA => mozjpeg::ColorSpace::JCS_EXT_RGBA,
            ColorSpace::RGBX => mozjpeg::ColorSpace::JCS_EXT_RGBX,
            ColorSpace::YCbCr => mozjpeg::ColorSpace::JCS_YCbCr,
            ColorSpace::BGR => mozjpeg::ColorSpace::JCS_EXT_BGR,
            ColorSpace::BGRA => mozjpeg::ColorSpace::JCS_EXT_BGRA,
            ColorSpace::BGRX => mozjpeg::ColorSpace::JCS_EXT_BGRX,
            ColorSpace::Unknown => mozjpeg::ColorSpace::JCS_UNKNOWN,
        }
    }
}

/// MozJpeg color processor
pub struct MozColorProcessor {
    colorspace: mozjpeg::ColorSpace,
    fancy_upsampling: bool,
    block_smoothing: bool,
    dct_method: DctMethod,
}

impl MozColorProcessor {
    pub fn new(
        colorspace: ColorSpace,
        fancy_upsampling: bool,
        block_smoothing: bool,
        dct_method: DctMethod,
    ) -> Self {
        Self {
            colorspace: colorspace.into(),
            fancy_upsampling,
            block_smoothing,
            dct_method,
        }
    }
}

impl ProcessorTrait<ColorPacket, ColorFrame> for MozColorProcessor {
    async fn process(&self, input: ColorPacket) -> Result<ColorFrame, Box<dyn Error>> {
        let mut decoder = Decompress::new_mem(&input.jpeg_buffer)?;

        decoder.do_fancy_upsampling(self.fancy_upsampling);
        decoder.do_block_smoothing(self.block_smoothing);
        decoder.dct_method(self.dct_method);

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

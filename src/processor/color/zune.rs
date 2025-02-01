use std::error::Error;

use zune_jpeg::{
    zune_core::{colorspace, options::DecoderOptions},
    JpegDecoder,
};

use crate::{processor::ProcessorTrait, COLOR_HEIGHT, COLOR_WIDTH};

use super::{ColorFrame, ColorPacket, ColorSpace};

impl From<colorspace::ColorSpace> for ColorSpace {
    fn from(value: colorspace::ColorSpace) -> Self {
        match value {
            colorspace::ColorSpace::RGB => Self::RGB,
            colorspace::ColorSpace::RGBA => Self::RGBA,
            colorspace::ColorSpace::YCbCr => Self::YCbCr,
            colorspace::ColorSpace::BGR => Self::BGR,
            colorspace::ColorSpace::BGRA => Self::BGRA,
            _ => Self::Unknown,
        }
    }
}

impl Into<colorspace::ColorSpace> for ColorSpace {
    fn into(self) -> colorspace::ColorSpace {
        match self {
            ColorSpace::RGB => colorspace::ColorSpace::RGB,
            ColorSpace::RGBA => colorspace::ColorSpace::RGBA,
            ColorSpace::YCbCr => colorspace::ColorSpace::YCbCr,
            ColorSpace::BGR => colorspace::ColorSpace::BGR,
            ColorSpace::BGRA => colorspace::ColorSpace::BGRA,
            ColorSpace::Unknown => colorspace::ColorSpace::Unknown,
        }
    }
}

/// ZuneJpeg color processor
pub struct ZuneColorProcessor(colorspace::ColorSpace);

impl ZuneColorProcessor {
    pub fn new(colorspace: ColorSpace) -> Self {
        Self(colorspace.into())
    }
}

impl ProcessorTrait<ColorPacket, ColorFrame> for ZuneColorProcessor {
    async fn process(&self, input: ColorPacket) -> Result<ColorFrame, Box<dyn Error>> {
        let mut decoder = JpegDecoder::new(input.jpeg_buffer);

        decoder.set_options(
            DecoderOptions::new_fast()
                .set_max_height(COLOR_HEIGHT)
                .set_max_width(COLOR_WIDTH)
                .jpeg_set_out_colorspace(self.0),
        );

        let buffer = decoder.decode()?;
        let dimensions = decoder.dimensions().expect("Expected dimensions");

        Ok(ColorFrame {
            color_space: decoder
                .get_output_colorspace()
                .expect("Expected colorspace")
                .into(),
            width: dimensions.0,
            height: dimensions.1,
            buffer,
            sequence: input.sequence,
            timestamp: input.timestamp,
            exposure: input.exposure,
            gain: input.gain,
            gamma: input.gamma,
        })
    }
}

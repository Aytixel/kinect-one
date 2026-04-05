use std::{error::Error, io::Cursor};

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

impl TryInto<colorspace::ColorSpace> for ColorSpace {
    type Error = &'static str;

    fn try_into(self) -> Result<colorspace::ColorSpace, Self::Error> {
        match self {
            ColorSpace::RGB => Ok(colorspace::ColorSpace::RGB),
            ColorSpace::RGBA => Ok(colorspace::ColorSpace::RGBA),
            ColorSpace::RGBX => Err("RGBX is not supported by ZuneJpeg"),
            ColorSpace::YCbCr => Ok(colorspace::ColorSpace::YCbCr),
            ColorSpace::BGR => Ok(colorspace::ColorSpace::BGR),
            ColorSpace::BGRA => Ok(colorspace::ColorSpace::BGRA),
            ColorSpace::BGRX => Err("BGRX is not supported by ZuneJpeg"),
            ColorSpace::Unknown => Ok(colorspace::ColorSpace::Unknown),
        }
    }
}

/// ZuneJpeg color processor
pub struct ZuneColorProcessor(colorspace::ColorSpace);

impl ZuneColorProcessor {
    pub fn new(colorspace: ColorSpace) -> Result<Self, Box<dyn Error>> {
        Ok(Self(colorspace.try_into()?))
    }
}

impl ProcessorTrait<ColorPacket, ColorFrame> for ZuneColorProcessor {
    async fn process(&self, input: ColorPacket) -> Result<ColorFrame, Box<dyn Error>> {
        let reader = Cursor::new(input.jpeg_buffer);
        let mut decoder = JpegDecoder::new(reader);

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
                .output_colorspace()
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

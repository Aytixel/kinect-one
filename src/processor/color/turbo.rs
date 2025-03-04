use std::error::Error;

use turbojpeg::{decompress, PixelFormat};

use crate::processor::ProcessorTrait;

use super::{ColorFrame, ColorSpace, ColorPacket};

impl From<PixelFormat> for ColorSpace {
    fn from(value: PixelFormat) -> Self {
        match value {
            PixelFormat::RGB => Self::RGB,
            PixelFormat::RGBA => Self::RGBA,
            PixelFormat::BGR => Self::BGR,
            PixelFormat::BGRA => Self::BGRA,
            _ => Self::Unknown,
        }
    }
}

impl TryInto<PixelFormat> for ColorSpace {
    type Error = &'static str;

    fn try_into(self) -> Result<PixelFormat, Self::Error> {
        match self {
            ColorSpace::RGB => Ok(PixelFormat::RGB),
            ColorSpace::RGBA => Ok(PixelFormat::RGBA),
            ColorSpace::YCbCr => Err("YCbCr is not supported by TurboJpeg"),
            ColorSpace::BGR => Ok(PixelFormat::BGR),
            ColorSpace::BGRA => Ok(PixelFormat::BGRA),
            ColorSpace::Unknown => Err("Unknown is not supported by TurboJpeg"),
        }
    }
}

/// TurboJpeg color processor
pub struct TurboColorProcessor(PixelFormat);

impl TurboColorProcessor {
    pub fn new(colorspace: ColorSpace) -> Result<Self, Box<dyn Error>> {
        Ok(Self(colorspace.try_into()?))
    }
}

impl ProcessorTrait<ColorPacket, ColorFrame> for TurboColorProcessor {
    async fn process(&self, input: ColorPacket) -> Result<ColorFrame, Box<dyn Error>> {
        let image = decompress(&input.jpeg_buffer, self.0)?;

        Ok(ColorFrame {
            color_space: image.format.into(),
            width: image.width,
            height: image.height,
            buffer: image.pixels,
            sequence: input.sequence,
            timestamp: input.timestamp,
            exposure: input.exposure,
            gain: input.gain,
            gamma: input.gamma,
        })
    }

    fn pipe<'a, 'b, T, P>(
        &'a self,
        processor: &'b P,
    ) -> crate::processor::PipedProcessor<'a, 'b, ColorPacket, ColorFrame, T, Self, P>
    where
        Self: Sized,
        P: ProcessorTrait<ColorFrame, T>,
    {
        crate::processor::PipedProcessor {
            _input: std::marker::PhantomData::default(),
            _tmp: std::marker::PhantomData::default(),
            _output: std::marker::PhantomData::default(),
            processor1: self,
            processor2: processor,
        }
    }
}

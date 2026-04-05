use std::error::Error;

use enough::Unstoppable;
use zenjpeg::decoder::{ChromaUpsampling, Decoder, PixelFormat};

use crate::{processor::ProcessorTrait, COLOR_HEIGHT, COLOR_WIDTH};

use super::{ColorFrame, ColorPacket, ColorSpace};

impl From<PixelFormat> for ColorSpace {
    fn from(value: PixelFormat) -> Self {
        match value {
            PixelFormat::Rgb => Self::RGB,
            PixelFormat::Rgba => Self::RGBA,
            PixelFormat::Bgr => Self::BGR,
            PixelFormat::Bgra => Self::BGRA,
            PixelFormat::Bgrx => Self::BGRX,
            _ => Self::Unknown,
        }
    }
}

impl TryInto<PixelFormat> for ColorSpace {
    type Error = &'static str;

    fn try_into(self) -> Result<PixelFormat, Self::Error> {
        match self {
            ColorSpace::RGB => Ok(PixelFormat::Rgb),
            ColorSpace::RGBA => Ok(PixelFormat::Rgba),
            ColorSpace::RGBX => Err("RGBX is not supported by TurboJpeg"),
            ColorSpace::YCbCr => Err("YCbCr is not supported by TurboJpeg"),
            ColorSpace::BGR => Ok(PixelFormat::Bgr),
            ColorSpace::BGRA => Ok(PixelFormat::Bgra),
            ColorSpace::BGRX => Ok(PixelFormat::Bgrx),
            ColorSpace::Unknown => Err("Unknown is not supported by TurboJpeg"),
        }
    }
}

/// ZenJpeg color processor
pub struct ZenColorProcessor {
    decoder: Decoder,
}

impl ZenColorProcessor {
    pub fn new(
        colorspace: ColorSpace,
        chroma_upsampling: ChromaUpsampling,
        dequant_bias: bool,
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            decoder: Decoder::new()
                .max_pixels((COLOR_WIDTH * COLOR_HEIGHT) as u64)
                .preserve_no_metadata()
                .chroma_upsampling(chroma_upsampling)
                .dequant_bias(dequant_bias)
                .output_format(colorspace.try_into()?),
        })
    }
}

impl ProcessorTrait<ColorPacket, ColorFrame> for ZenColorProcessor {
    async fn process(&self, input: ColorPacket) -> Result<ColorFrame, Box<dyn Error>> {
        let decoder_result = self.decoder.decode(&input.jpeg_buffer, Unstoppable)?;

        Ok(ColorFrame {
            color_space: decoder_result.format().into(),
            width: decoder_result.width() as usize,
            height: decoder_result.height() as usize,
            buffer: decoder_result.into_pixels_u8().unwrap_or_default(),
            sequence: input.sequence,
            timestamp: input.timestamp,
            exposure: input.exposure,
            gain: input.gain,
            gamma: input.gamma,
        })
    }
}

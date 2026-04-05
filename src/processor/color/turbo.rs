use std::error::Error;

use turbojpeg::{yuv_pixels_len, Decompressor, Image, PixelFormat, YuvImage};

use crate::processor::ProcessorTrait;

use super::{ColorFrame, ColorPacket, ColorSpace};

impl From<Option<PixelFormat>> for ColorSpace {
    fn from(value: Option<PixelFormat>) -> Self {
        match value {
            Some(PixelFormat::RGB) => Self::RGB,
            Some(PixelFormat::RGBA) => Self::RGBA,
            Some(PixelFormat::RGBX) => Self::RGBX,
            Some(PixelFormat::BGR) => Self::BGR,
            Some(PixelFormat::BGRA) => Self::BGRA,
            Some(PixelFormat::BGRX) => Self::BGRX,
            None => Self::YCbCr,
            _ => Self::Unknown,
        }
    }
}

impl TryInto<Option<PixelFormat>> for ColorSpace {
    type Error = &'static str;

    fn try_into(self) -> Result<Option<PixelFormat>, Self::Error> {
        match self {
            ColorSpace::RGB => Ok(Some(PixelFormat::RGB)),
            ColorSpace::RGBA => Ok(Some(PixelFormat::RGBA)),
            ColorSpace::RGBX => Ok(Some(PixelFormat::RGBX)),
            ColorSpace::YCbCr => Ok(None),
            ColorSpace::BGR => Ok(Some(PixelFormat::BGR)),
            ColorSpace::BGRA => Ok(Some(PixelFormat::BGRA)),
            ColorSpace::BGRX => Ok(Some(PixelFormat::BGRX)),
            ColorSpace::Unknown => Err("Unknown is not supported by TurboJpeg"),
        }
    }
}

/// TurboJpeg color processor
pub struct TurboColorProcessor {
    color_space: Option<PixelFormat>,
}

impl TurboColorProcessor {
    pub fn new(colorspace: ColorSpace) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            color_space: colorspace.try_into()?,
        })
    }
}

impl ProcessorTrait<ColorPacket, ColorFrame> for TurboColorProcessor {
    async fn process(&self, input: ColorPacket) -> Result<ColorFrame, Box<dyn Error>> {
        let mut decompressor = Decompressor::new()?;
        let header = decompressor.read_header(&input.jpeg_buffer)?;

        let pixels = if let Some(color_space) = self.color_space {
            let pitch = header.width * color_space.size();
            let mut image = Image {
                pixels: vec![0; header.height * pitch],
                width: header.width,
                pitch,
                height: header.height,
                format: color_space,
            };

            decompressor.decompress(&input.jpeg_buffer, image.as_deref_mut())?;

            image.pixels
        } else {
            let align = 4;
            let yuv_pixels_len =
                yuv_pixels_len(header.width, align, header.height, header.subsamp)?;
            let mut yuv_image = YuvImage {
                pixels: vec![0; yuv_pixels_len],
                width: header.width,
                align,
                height: header.height,
                subsamp: header.subsamp,
            };

            decompressor.decompress_to_yuv(&input.jpeg_buffer, yuv_image.as_deref_mut())?;

            yuv_image.pixels
        };

        Ok(ColorFrame::from_packet(
            self.color_space.into(),
            pixels,
            &input,
        ))
    }
}

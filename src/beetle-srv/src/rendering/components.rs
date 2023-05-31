use qrencode as qrcode;
use serde::{Deserialize, Serialize};
use std::io;

use super::{constants, fonts};

/// Wraps a string _and_ a font.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct StylizedMessage<S> {
  /// The text to draw.
  pub message: S,

  /// The font to use.
  pub font: fonts::FontSelection,

  /// The scale to apply to our font.
  pub size: Option<f32>,
}

impl<S> StylizedMessage<S>
where
  S: std::convert::AsRef<str>,
{
  /// Attempts to draw the message at a location on an image. Returns the dimensions of the bounding
  /// box for this element.
  pub(super) fn draw<C>(&self, location: (i32, i32), image: &mut C) -> io::Result<(i32, i32)>
  where
    C: imageproc::drawing::Canvas<Pixel = image::Luma<u8>>,
  {
    use constants::MESSAGE_LAYOUT_BOUNDING_CONSTANTS as BOUNDING;

    // Render our font.
    let font = Vec::from(self.font.bytes());
    let font = rusttype::Font::try_from_vec(font).ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::Other,
        "Unable to build valid font context for rasterizing",
      )
    })?;
    let scale = rusttype::Scale {
      x: self.size.unwrap_or(80f32),
      y: self.size.unwrap_or(80f32),
    };
    let text_dimensions = imageproc::drawing::text_size(scale, &font, self.message.as_ref());

    let (mut width, mut height) = (text_dimensions.0, text_dimensions.1);

    if text_dimensions.0 > 0 && text_dimensions.1 > 0 {
      let (bounding_rect, inner_rect) = BOUNDING.border_rectangles(location, text_dimensions);

      height = bounding_rect.height() as i32;
      width = bounding_rect.width() as i32;

      imageproc::drawing::draw_filled_rect_mut(image, bounding_rect, image::Luma([0]));
      imageproc::drawing::draw_filled_rect_mut(image, inner_rect, image::Luma([255]));
    }

    imageproc::drawing::draw_text_mut(
      image,
      image::Luma([0]),
      location.0,
      location.1,
      scale,
      &font,
      self.message.as_ref(),
    );

    Ok((width, height))
  }
}

/// Wraps a string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Scannable<S> {
  /// The thing to make a qr code of.
  pub contents: S,
}

impl<S> Scannable<S>
where
  S: AsRef<str>,
{
  /// Attempts to produce a grayscale image of the provided dimensions for the contents held by it.
  pub(super) fn grayscale(&self, dimensions: (u32, u32)) -> io::Result<image::GrayImage> {
    let str_msg = self.contents.as_ref();

    let code = qrcode::QrCode::new(str_msg.as_bytes()).map_err(|error| {
      log::warn!("unable to create QR code from '{str_msg}' - {error}");
      io::Error::new(io::ErrorKind::Other, format!("{error}"))
    })?;

    Ok(
      code
        .render::<image::Luma<u8>>()
        .max_dimensions(dimensions.0, dimensions.1)
        .build(),
    )
  }
}

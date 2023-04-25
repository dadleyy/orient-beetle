use serde::{Deserialize, Serialize};
use std::io;

/// Compile-time loading of our `.ttf` data.
const TEXT_FONT: &[u8] = include_bytes!("../../DejaVuSans.ttf");

/// The rendering queue module contains the central business logic for taking a layout and adding
/// it to the queue of things to be rendered and sent to devices.
pub mod queue;

/// The renderer itself is responsible for periodically popping from the queue and doing the
/// things.
pub mod renderer;

/// The render layout represents the various kinds of layouts that can be rendered into a
/// rasterized image and sent to the embedded devices.
#[derive(Debug, Serialize, Deserialize)]
pub enum RenderLayout<S> {
  /// The simplest form of render layout - a single message.
  Message(S),
}

impl<S> RenderLayout<S>
where
  S: std::convert::AsRef<str>,
{
  /// Turn this layout into a rasterized image.
  pub fn rasterize(self, dimensions: (u32, u32)) -> io::Result<Vec<u8>> {
    match self {
      Self::Message(message) => {
        let mut image = image::GrayImage::new(dimensions.0, dimensions.1);
        imageproc::drawing::draw_filled_rect_mut(
          &mut image,
          imageproc::rect::Rect::at(0, 0).of_size(dimensions.0, dimensions.1),
          image::Luma([255]),
        );
        let mut formatted_buffer = std::io::Cursor::new(Vec::with_capacity((dimensions.0 * dimensions.1) as usize));
        let font = Vec::from(TEXT_FONT);
        let font = rusttype::Font::try_from_vec(font).ok_or_else(|| {
          io::Error::new(
            io::ErrorKind::Other,
            "Unable to build valid font context for rasterizing",
          )
        })?;
        let height = 50f32;
        let scale = rusttype::Scale { x: height, y: height };
        imageproc::drawing::draw_text_mut(&mut image, image::Luma([0]), 0, 0, scale, &font, message.as_ref());

        image
          .write_to(&mut formatted_buffer, image::ImageOutputFormat::Png)
          .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("unable to build image: {error}")))?;

        Ok(formatted_buffer.into_inner())
      }
    }
  }
}

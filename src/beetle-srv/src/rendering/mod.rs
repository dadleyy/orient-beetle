use serde::{Deserialize, Serialize};
use std::io;

use qrencode as qrcode;

/// Compile-time loading of our `.ttf` data.
const TEXT_FONT: &[u8] = include_bytes!("../../DejaVuSans.ttf");

/// The rendering queue module contains the central business logic for taking a layout and adding
/// it to the queue of things to be rendered and sent to devices.
pub mod queue;
pub use queue::QueuedRenderAuthority;

/// The renderer itself is responsible for periodically popping from the queue and doing the
/// things.
pub mod renderer;

/// Wraps a string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RenderMessageLayout<S> {
  /// The text to draw.
  pub message: S,
}

/// Wraps a string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RenderScannableLayout<S> {
  /// The thing to make a qr code of.
  pub contents: S,
}

/// The render layout represents the various kinds of layouts that can be rendered into a
/// rasterized image and sent to the embedded devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum RenderLayout<S> {
  /// The simplest form of render layout - a single message.
  Message(RenderMessageLayout<S>),

  /// The simplest form of render layout - a single message.
  Scannable(RenderScannableLayout<S>),
}

impl<S> RenderLayout<S>
where
  S: std::convert::AsRef<str>,
{
  /// Turn this layout into a rasterized image.
  pub fn rasterize(self, dimensions: (u32, u32)) -> io::Result<Vec<u8>> {
    match self {
      Self::Scannable(RenderScannableLayout { contents: message }) => {
        let str_msg = message.as_ref();
        let code = qrcode::QrCode::new(str_msg.as_bytes()).map_err(|error| {
          log::warn!("unable to create QR code from '{str_msg}' - {error}");
          io::Error::new(io::ErrorKind::Other, format!("{error}"))
        })?;

        let image = code
          .render::<image::Luma<u8>>()
          .max_dimensions(dimensions.0, dimensions.1)
          .build();

        let mut formatted_buffer = std::io::Cursor::new(Vec::with_capacity((dimensions.0 * dimensions.1) as usize));

        image
          .write_to(&mut formatted_buffer, image::ImageOutputFormat::Png)
          .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("unable to build image: {error}")))?;

        Ok(formatted_buffer.into_inner())
      }

      Self::Message(RenderMessageLayout { message }) => {
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
        let height = 80f32;
        let scale = rusttype::Scale { x: height, y: height };
        imageproc::drawing::draw_text_mut(&mut image, image::Luma([0]), 10, 10, scale, &font, message.as_ref());

        image
          .write_to(&mut formatted_buffer, image::ImageOutputFormat::Png)
          .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("unable to build image: {error}")))?;

        Ok(formatted_buffer.into_inner())
      }
    }
  }
}

/// "Rendering" commands associated with the rgb lights on a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightingLayout {
  /// Requests the device to turn the lights off.
  Off,
  /// Requests the device to turn the lights on.
  On,
}

/// Wraps the contents of our outermost enum. It helps serde with more straightforward
/// serialization.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct RenderLayoutContainer<S> {
  /// The wrapped layout
  pub layout: S,
}

/// Wraps the lighting and display of the device.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum RenderVariant<S> {
  /// Requests a layout be rendered to the screen.
  Layout(RenderLayoutContainer<RenderLayout<S>>),
  /// Requests some change in the lighting.
  Lighting(RenderLayoutContainer<LightingLayout>),
}

impl<S> RenderVariant<S> {
  pub fn scannable(contents: S) -> Self {
    let layout = RenderLayout::Scannable(RenderScannableLayout { contents });
    Self::Layout(RenderLayoutContainer { layout })
  }
}

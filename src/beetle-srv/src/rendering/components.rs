use qrencode as qrcode;
use serde::{Deserialize, Serialize};
use std::io;

use super::fonts;

/// A bounding box where everything is optional.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct OptionalBoundingBox {
  /// The optional left of this box.
  pub left: Option<i32>,
  /// The optional right of this box.
  pub right: Option<i32>,
  /// The optional top of this box.
  pub top: Option<i32>,
  /// The optional bottom of this box.
  pub bottom: Option<i32>,
}

/// Wraps a string _and_ a font.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct StylizedMessage<S> {
  /// The text to draw.
  pub message: S,

  /// The font to use.
  pub font: Option<fonts::FontSelection>,

  /// The amount of padding, if any.
  pub padding: Option<OptionalBoundingBox>,

  /// The amount of padding, if any.
  pub margin: Option<OptionalBoundingBox>,

  /// The amount of border, if any.
  pub border: Option<OptionalBoundingBox>,

  /// The scale to apply to our font.
  pub size: Option<f32>,
}

impl<S> Default for StylizedMessage<S>
where
  S: Default,
{
  fn default() -> Self {
    let font = fonts::FontSelection::default();
    Self {
      message: S::default(),
      font: Some(font),
      size: None,
      margin: None,
      padding: None,
      border: None,
    }
  }
}

/// The kinds of constraints that can be applied to a message bounding box.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub(crate) enum StylizedMessageBoundingConstraints {
  /// Tells the text rendering engine to truncate after the width value.
  MaxWidth(i32),
}

/// A clipping box where a stylized message should be rendered within.
pub(crate) struct StylizedMessageBounding {
  /// The left-most location of the message.
  pub(crate) left: i32,
  /// The top location of the message.
  pub(crate) top: i32,

  /// An optional constraint to apply to message.
  pub(crate) constraints: Option<StylizedMessageBoundingConstraints>,
}

impl<S> StylizedMessage<S>
where
  S: std::convert::AsRef<str>,
{
  /// Render the text within this bounding box.
  pub(super) fn draw_within<C>(&self, bounds: &StylizedMessageBounding, image: &mut C) -> io::Result<(i32, i32)>
  where
    C: imageproc::drawing::Canvas<Pixel = image::Luma<u8>>,
  {
    let df = fonts::FontSelection::default();
    // Get our font from the embedded ttf data.
    let font = self.font.as_ref().unwrap_or(&df).renderer()?;
    let scale = rusttype::Scale {
      x: self.size.unwrap_or(80f32),
      y: self.size.unwrap_or(80f32),
    };

    let mb = self.margin.as_ref().and_then(|m| m.bottom).unwrap_or(0);
    let mt = self.margin.as_ref().and_then(|m| m.top).unwrap_or(0);
    let ml = self.margin.as_ref().and_then(|m| m.left).unwrap_or(0);
    let mr = self.margin.as_ref().and_then(|m| m.right).unwrap_or(0);

    let pl = self.padding.as_ref().and_then(|m| m.left).unwrap_or(0);
    let pr = self.padding.as_ref().and_then(|m| m.right).unwrap_or(0);
    let pt = self.padding.as_ref().and_then(|m| m.top).unwrap_or(0);
    let pb = self.padding.as_ref().and_then(|m| m.bottom).unwrap_or(0);

    let top = bounds.top + mt;
    let left = bounds.left + ml;

    let mut rendered_text = std::borrow::Cow::from(self.message.as_ref());
    let mut message = self.message.as_ref();
    let mut text_dimensions = imageproc::drawing::text_size(scale, &font, message);

    if let Some(StylizedMessageBoundingConstraints::MaxWidth(max_width)) = bounds.constraints {
      let mut trunc_size = 0;
      let mut cloned = message.to_string();

      while (text_dimensions.0 + ml + pl) > max_width {
        (!message.is_empty()).then_some(()).ok_or_else(|| {
          io::Error::new(
            io::ErrorKind::Other,
            format!("unable to render '{}' within {max_width}", self.message.as_ref()),
          )
        })?;

        (message, _) = message.split_at(message.len() - 1);
        text_dimensions = imageproc::drawing::text_size(scale, &font, message);
        trunc_size += 1;
      }

      if trunc_size > 0 {
        (message, _) = message.split_at(message.len() - 3);
        cloned = format!("{message}...");
      }

      rendered_text = std::borrow::Cow::from(cloned);
    }

    // Currently, this will _not_ take into account how wide the text is.
    if let Some(border) = self.border.as_ref() {
      let bh = text_dimensions.1 + pt + pb;
      let bw = text_dimensions.0 + pl + pr;
      let bl = left + border.left.as_ref().copied().unwrap_or(0);

      let bounding_rect = imageproc::rect::Rect::at(left, top).of_size(bw as u32, bh as u32);
      let inner_rect = imageproc::rect::Rect::at(bl, top).of_size(bw as u32, bh as u32);
      imageproc::drawing::draw_filled_rect_mut(image, bounding_rect, image::Luma([0]));
      imageproc::drawing::draw_filled_rect_mut(image, inner_rect, image::Luma([255]));
    }

    imageproc::drawing::draw_text_mut(
      image,
      image::Luma([0]),
      left + pl,
      top + pt,
      scale,
      &font,
      &rendered_text,
    );

    let height = text_dimensions.1 + mt + mb + pt + pb;
    let width = text_dimensions.0 + ml + mr + pl + pr;
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

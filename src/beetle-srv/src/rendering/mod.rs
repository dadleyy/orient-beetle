//! Note: the actually rendering here - the process of generating a bitmap image to send to the
//! device based on some data structure - could be way more advanced than it is currently. The
//! `imageproc` crate is very nice for the time being, but it may not support the wild kinds of
//! rendering that this would want to support in the future.

use serde::{Deserialize, Serialize};
use std::io;

/// Defines all of the binary-included fonts available.
mod fonts;
pub use fonts::FontSelection;

/// Defines layout constants like padding and margins.
mod constants;

/// Defines the components that can be used within a layout.
pub mod components;
pub use components::{OptionalBoundingBox, StylizedMessage};

/// The rendering queue module contains the central business logic for taking a layout and adding
/// it to the queue of things to be rendered and sent to devices.
pub(crate) mod queue;
pub use queue::{Queue, QueuedRenderAuthority};

/// The renderer itself is responsible for periodically popping from the queue and doing the
/// things.
mod renderer;
pub use renderer::run;

/// The types of things that a split can contain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum SplitContents<S> {
  /// A list of messages.
  Messages(Vec<components::StylizedMessage<S>>),

  /// Embeds a qr code onto the split side.
  Scannable(components::Scannable<S>),
}

/// An layout that has content on the left and right.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SplitLayout<S> {
  /// What gets rendered on the left.
  pub left: SplitContents<S>,

  /// What gets rendered on the right.
  pub right: SplitContents<S>,

  /// A 0-100 value of how much space the _left_ split should take.
  pub ratio: u8,
}

/// The render layout represents the various kinds of layouts that can be rendered into a
/// rasterized image and sent to the embedded devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum RenderLayout<S> {
  /// Clears the screen.
  Clear,

  /// A single styleized message. Will be rendered in the middle of the display being rasterized
  /// to.
  StylizedMessage(components::StylizedMessage<S>),

  /// A layout that has content on the left, and content on the right.
  Split(SplitLayout<S>),

  /// A single qr code that will be rendered to the whole dimensions.
  Scannable(components::Scannable<S>),
}

impl<S> RenderLayout<S>
where
  S: std::convert::AsRef<str>,
{
  /// Turn this layout into a rasterized image.
  pub fn rasterize(self, dimensions: (u32, u32)) -> io::Result<Vec<u8>> {
    let mut image = image::GrayImage::new(dimensions.0, dimensions.1);

    if dimensions.0 > constants::MAX_WIDTH || dimensions.1 > constants::MAX_HEIGHT {
      return Err(io::Error::new(
        io::ErrorKind::Other,
        "dimensions exceed reasonable resolution",
      ));
    }

    // Start with an entirely white background.
    imageproc::drawing::draw_filled_rect_mut(
      &mut image,
      imageproc::rect::Rect::at(0, 0).of_size(dimensions.0, dimensions.1),
      image::Luma([255]),
    );

    match self {
      Self::Clear => (),
      // Scannables will replace the image buffer with one provided from the qr crate.
      Self::Scannable(scannable) => {
        image = scannable.grayscale(dimensions)?;
      }

      Self::Split(SplitLayout { left, right, ratio }) => {
        let left_max = match ratio {
          25 => dimensions.0 / 4,
          33 => dimensions.0 / 3,
          50 => dimensions.0 / 2,
          66 => dimensions.0 - (dimensions.0 / 3),
          75 => dimensions.0 - (dimensions.0 / 4),
          80 => dimensions.0 - (dimensions.0 / 5),
          // TODO: support more breakpoints, or do actual math. This is just implemented this way
          // for quick, strict support.
          _ => dimensions.0 / 2,
        };

        match left {
          SplitContents::Messages(messages) => {
            let mut top = 0;
            for m in messages {
              let bounds = components::StylizedMessageBounding {
                left: 0,
                top,
                constraints: Some(components::StylizedMessageBoundingConstraints::MaxWidth(
                  left_max as i32,
                )),
              };
              let (_, h) = m.draw_within(&bounds, &mut image)?;
              top += h;
            }
          }
          _ => todo!(),
        }

        match right {
          SplitContents::Messages(messages) => {
            let mut top = 0;
            for m in messages {
              let bounds = components::StylizedMessageBounding {
                left: left_max as i32,
                top,
                constraints: Some(components::StylizedMessageBoundingConstraints::MaxWidth(
                  (dimensions.0 - left_max) as i32,
                )),
              };
              let (_, h) = m.draw_within(&bounds, &mut image)?;
              top += h;
            }
          }
          _ => todo!(),
        }
      }

      // If we're just a stylized image, draw us.
      Self::StylizedMessage(message_layout) => {
        let bounding = components::StylizedMessageBounding {
          left: 10,
          top: 10,
          constraints: None,
        };
        message_layout.draw_within(&bounding, &mut image)?;
      }
    }

    // Create our output buffer and write the image into it.
    let mut formatted_buffer = std::io::Cursor::new(Vec::with_capacity((dimensions.0 * dimensions.1) as usize));
    image
      .write_to(&mut formatted_buffer, image::ImageOutputFormat::Png)
      .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("unable to build image: {error}")))?;
    Ok(formatted_buffer.into_inner())
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

  /// When this layout was created.
  pub created: Option<chrono::DateTime<chrono::Utc>>,
}

/// Wraps the lighting and display of the device.
///
/// Note that the fact that both lighting and actual display control is handled by this single type
/// seems like it could be particularly confusing for folks from the outside, and it is likely that
/// renaming this and/or separating them makes the most sense.
///
/// Since both lighting and display are ultimately serialized onto the same device "rendering"
/// queue, they live like this for now.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum RenderVariant<S> {
  /// Requests a layout be rendered to the screen.
  Layout(RenderLayoutContainer<RenderLayout<S>>),

  /// Requests some change in the lighting.
  Lighting(RenderLayoutContainer<LightingLayout>),
}

impl<S> RenderVariant<S> {
  /// Helper constructor for turning lights on.
  pub fn on() -> Self {
    Self::Lighting(RenderLayoutContainer {
      created: Some(chrono::Utc::now()),
      layout: LightingLayout::On,
    })
  }

  /// Helper constructor for turning lights off.
  pub fn off() -> Self {
    Self::Lighting(RenderLayoutContainer {
      created: Some(chrono::Utc::now()),
      layout: LightingLayout::Off,
    })
  }

  /// A helper "type constructor" that will wrap the deep-inner scannable content in the
  /// container types.
  pub fn scannable(contents: S) -> Self {
    let layout = RenderLayout::Scannable(components::Scannable { contents });
    let created = Some(chrono::Utc::now());
    Self::Layout(RenderLayoutContainer { layout, created })
  }

  /// Helper type constructor
  pub fn layout(layout: RenderLayout<S>) -> Self {
    let created = Some(chrono::Utc::now());
    Self::Layout(RenderLayoutContainer { layout, created })
  }

  /// Helper type constructor
  pub fn message(message: S) -> Self {
    #[allow(deprecated)]
    let layout = RenderLayout::StylizedMessage(components::StylizedMessage {
      border: None,
      margin: None,
      padding: None,
      font: Some(fonts::FontSelection::Barlow),
      message,
      size: None,
    });
    let created = Some(chrono::Utc::now());
    Self::Layout(RenderLayoutContainer { layout, created })
  }
}

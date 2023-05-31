use serde::{Deserialize, Serialize};
use std::io;

/// Defines all of the binary-included fonts available.
mod fonts;

/// Defines layout constants like padding and margins.
mod constants;

/// Defines the components that can be used within a layout.
pub mod components;

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
}

/// The render layout represents the various kinds of layouts that can be rendered into a
/// rasterized image and sent to the embedded devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
pub enum RenderLayout<S> {
  /// The simplest form of render layout - a single message.
  #[deprecated(note = "will hopefully be removed as layouts are migrated to formal definitions.")]
  Message(RenderMessageLayout<S>),

  /// Added complexity - font selection.
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

    // Start with an entirely white background.
    imageproc::drawing::draw_filled_rect_mut(
      &mut image,
      imageproc::rect::Rect::at(0, 0).of_size(dimensions.0, dimensions.1),
      image::Luma([255]),
    );

    match self {
      // Scannables will replace the image buffer with one provided from the qr crate.
      Self::Scannable(scannable) => {
        image = scannable.grayscale(dimensions)?;
      }

      Self::Split(SplitLayout { left, right }) => {
        use constants::MESSAGE_LAYOUT_BOUNDING_CONSTANTS as BOUNDING;
        let (start_x, mut start_y) = (BOUNDING.content_margin, BOUNDING.content_margin);
        let mut rightmost_left = start_x;

        match left {
          SplitContents::Scannable(s) => {
            let code = s.grayscale((dimensions.0 / 2, dimensions.1))?;
            rightmost_left = (dimensions.0 / 2) as i32;
            image::GenericImage::copy_from(&mut image, &code, 0, 0)
              .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("cannot copy scannable - {error}")))?;
          }
          SplitContents::Messages(messages) => {
            for stylized in messages {
              let dimensions = stylized.draw((start_x, start_y), &mut image)?;

              if dimensions.0 > rightmost_left {
                rightmost_left = dimensions.0;
              }

              start_y += dimensions.1;
            }
          }
        }

        // Add the starting position onto our rightmost left.
        rightmost_left += start_x;
        start_y = BOUNDING.content_margin;

        match right {
          SplitContents::Scannable(s) => {
            let code_width = dimensions.0 - (rightmost_left as u32);
            let code_height = std::cmp::min(dimensions.1, code_width);
            let code = s.grayscale((code_width, code_height))?;

            image::GenericImage::copy_from(&mut image, &code, rightmost_left as u32, 0)
              .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("cannot copy scannable - {error}")))?;
          }
          SplitContents::Messages(messages) => {
            for stylized in messages {
              let dimensions = stylized.draw((rightmost_left, start_y), &mut image)?;
              start_y += dimensions.1;
            }
          }
        }
      }

      // If we're just a stylized image, draw us.
      Self::StylizedMessage(message_layout) => {
        use constants::MESSAGE_LAYOUT_BOUNDING_CONSTANTS as BOUNDING;
        message_layout.draw((BOUNDING.content_margin, BOUNDING.content_margin), &mut image)?;
      }

      // If we're not a stylized image, use defaults and draw.
      #[allow(deprecated)]
      Self::Message(RenderMessageLayout { message }) => {
        return Self::StylizedMessage(components::StylizedMessage {
          message,
          font: fonts::FontSelection::DejaVu,
          size: None,
        })
        .rasterize(dimensions)
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
    let layout = RenderLayout::Message(RenderMessageLayout { message });
    let created = Some(chrono::Utc::now());
    Self::Layout(RenderLayoutContainer { layout, created })
  }
}

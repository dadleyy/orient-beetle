/// The type used to define some compile-time dimensional constants.
pub(super) struct BoundingConstants {
  /// The amount of border to place around the content.
  pub(super) border_width: i32,
  /// The amount of space between outside of border and content.
  pub(super) content_padding: i32,
  /// The amount of space between content and screen edge.
  pub(super) content_margin: i32,
}

impl BoundingConstants {
  /// Computes the two rectangles needed to produce a border of the appropriate width surrounding
  /// the provided dimensions.
  pub fn border_rectangles(
    &self,
    content_location: (i32, i32),
    content_dimensions: (i32, i32),
  ) -> (imageproc::rect::Rect, imageproc::rect::Rect) {
    let full_padding = self.content_padding * 2;
    let full_border = self.border_width * 2;

    let bounding_width = content_dimensions.0 + full_padding;
    let bounding_height = content_dimensions.1 + full_padding;

    let inner_width = bounding_width - full_border;
    let inner_height = bounding_height - full_border;

    let (x, y) = (
      content_location.0 - self.content_padding,
      content_location.1 - self.content_padding,
    );
    let (ix, iy) = (x + self.border_width, y + self.border_width);

    let bounding_rect = imageproc::rect::Rect::at(x, y).of_size(bounding_width as u32, bounding_height as u32);
    let inner_rect = imageproc::rect::Rect::at(ix, iy).of_size(inner_width as u32, inner_height as u32);

    (bounding_rect, inner_rect)
  }
}

/// The most basic instance of content layout defaults.
pub(super) const MESSAGE_LAYOUT_BOUNDING_CONSTANTS: BoundingConstants = BoundingConstants {
  border_width: 2,
  content_padding: 10,
  content_margin: 20,
};

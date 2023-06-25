use serde::{Deserialize, Serialize};
use std::io;

/// Teko @ `<https://fonts.google.com/specimen/Teko?preview.text=2:00PM&preview.text_type=custom>`
const TEKO: &[u8] = include_bytes!("../../Teko-Regular.ttf");

/// Teko @ `<https://fonts.google.com/specimen/Barlow?preview.text=2:00PM%20-%204:00PM&preview.layout=row&preview.text_type=custom>`
const BARLOW: &[u8] = include_bytes!("../../Barlow-Regular.ttf");

/// DejaVu Sans @ `<https://dejavu-fonts.github.io/>`
const DEJAVU_SANS: &[u8] = include_bytes!("../../DejaVuSans.ttf");

/// Roboto Sans @ `<https://fonts.google.com/specimen/Roboto>`
const ROBOTO_SANS: &[u8] = include_bytes!("../../Roboto-Regular.ttf");

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
/// Enumerates the fonts available to us.
pub enum FontSelection {
  /// See constant.
  #[default]
  Roboto,

  /// See constant.
  Teko,

  /// See constant.
  Barlow,

  /// See constant.
  DejaVu,
}

impl FontSelection {
  /// Returns the compile-time memory location of the font matching the selection.
  pub fn bytes(&self) -> &'static [u8] {
    match self {
      Self::Barlow => BARLOW,
      Self::DejaVu => DEJAVU_SANS,
      Self::Roboto => ROBOTO_SANS,
      Self::Teko => TEKO,
    }
  }

  /// Creates the font rendering object.
  pub fn renderer(&self) -> io::Result<rusttype::Font> {
    let font = Vec::from(self.bytes());

    rusttype::Font::try_from_vec(font).ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::Other,
        "Unable to build valid font context for rasterizing",
      )
    })
  }
}

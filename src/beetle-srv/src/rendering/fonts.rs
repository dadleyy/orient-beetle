use serde::{Deserialize, Serialize};

/// DejaVu Sans @ https://dejavu-fonts.github.io/
const DEJAVU_SANS: &[u8] = include_bytes!("../../DejaVuSans.ttf");

/// Roboto Sans @ https://fonts.google.com/specimen/Roboto
const ROBOTO_SANS: &[u8] = include_bytes!("../../Roboto-Regular.ttf");

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "beetle:kind", content = "beetle:content")]
/// Enumerates the fonts available to us.
pub enum FontSelection {
  /// See constant.
  Roboto,

  /// See constant.
  #[default]
  DejaVu,
}

impl FontSelection {
  /// Returns the compile-time memory location of the font matching the selection.
  pub fn bytes(&self) -> &'static [u8] {
    match self {
      Self::DejaVu => DEJAVU_SANS,
      Self::Roboto => ROBOTO_SANS,
    }
  }
}

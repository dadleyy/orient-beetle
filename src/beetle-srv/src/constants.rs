// LIST: stores all of the available ids that devices can pop from.
pub const REGISTRAR_AVAILABLE: &'static str = "ob:r";

// LIST: devices push into this every time they pop from their individual keys.
pub const REGISTRAR_INCOMING: &'static str = "ob:i";

// HASH: a mapping of `{ [id]: ... }`.
pub const REGISTRAR_ACTIVE: &'static str = "ob:a";
pub const REGISTRAR_INDEX: &'static str = "ob:s";

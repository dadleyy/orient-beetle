/// LIST: stores all of the available ids that devices can pop from.
pub const REGISTRAR_AVAILABLE: &str = "ob:r";

/// LIST: devices push into this every time they pop from their individual keys.
pub const REGISTRAR_INCOMING: &str = "ob:i";

/// SET: an index of all active device ids.
pub const REGISTRAR_INDEX: &str = "ob:s";

/// LIST: stores all of the available ids that devices can pop from.
pub const REGISTRAR_AVAILABLE: &str = "ob:r";

/// LIST: devices push into this every time they pop from their individual keys.
pub const REGISTRAR_INCOMING: &str = "ob:i";

/// SET: an index of all active device ids.
pub const REGISTRAR_INDEX: &str = "ob:s";

/// LIST: rendering queue.
pub const RENDERING_QUEUE: &str = "ob:rendering";

/// LIST: general registrar job queue.
pub const REGISTRAR_JOB_QUEUE: &str = "ob:registrar-jobs";

/// HASH:  registrar job queue.
pub const REGISTRAR_JOB_RESULTS: &str = "ob:registrar-job-results";

/// The prefix used for lighting command messages.
pub const LIGHTING_PREFIX: &str = "lighting";

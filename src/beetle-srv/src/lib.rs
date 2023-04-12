#![deny(unsafe_code, clippy::missing_docs_in_private_items)]

//! This crate provides the functionality shared across the binaries provided by the project at
//! large.

/// The HTTP/JSON api using `tide`.
pub mod api;

/// The configuration we publically expose from this crate that can be deserialized from various
/// formats.
pub mod config;

/// Constants available. May be more appropriate on a per-module basis.
pub mod constants;

/// Random, unqiue identifier helpers.
pub mod identity;

/// Mongo functionality.
pub mod mongo;

/// Redis functionality.
pub mod redis;

/// Functionality associated with our background registrar worker.
pub mod registrar;

/// Generally speaking, this module contains types stored in the mongo instance. It should be
/// renamed to something better.
pub mod types;

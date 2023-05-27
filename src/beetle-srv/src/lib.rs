#![deny(unsafe_code, clippy::missing_docs_in_private_items)]

//! This crate provides the functionality shared across the binaries provided by the project at
//! large. Those binaries are:
//!
//! 1. [`beetle_cli`][cli] - a command line tool for platform administration.
//!
//! 2. [`beetle_web`][web] - the [tide] web api that is consumed by the browser-side, ui
//!    application.
//!
//! 3. [`beetle_registrar`][reg] - the "registrar" which is responsible for handling delayed "jobs"
//!    created from the web application, as well as managing the available device id pool and
//!    processing messages received from them.
//!
//! 4. [`beetle_renderer`][ren] - the background process that is responsible for actually creating
//!    png images based on some requested layout.
//!
//! [reg]: ../beetle_registrar/index.html
//! [web]: ../beetle_web/index.html
//! [cli]: ../beetle_web/index.html
//! [ren]: ../beetle_renderer/index.html

/// The HTTP/JSON api using `tide`.
pub mod api;

/// The functionality associated with building images.
pub mod rendering;

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

/// This module contains the enumerated types related to the various jobs performed in our system.
mod job_result;

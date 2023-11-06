//! This module holds all of the api-specific functionality, including the routes and the
//! payloads/responses they are concerned with.

use serde::{Deserialize, Serialize};

/// The api for authenticating.
mod auth;

/// The api for claiming access to devices.
mod claims;

/// The device api routes module.
mod devices;

/// The schedule api routes module.
mod schedules;

/// The main worker module.
mod worker;

/// Routes related to the job result store;
mod jobs;

pub use worker::Worker;

/// These configuration definitions makes it easy for the web binary to
/// deserialize a configuration file (e.g toml) and have everything ready
/// for the server to run.

/// Web configuration.
#[derive(Deserialize, Clone)]
pub struct WebConfiguration {
  /// The location on disc where files should be saved temporarily.
  temp_file_storage: String,
  /// The domain to associated cookies with.
  cookie_domain: String,
  /// Where to send folks after the Oauth handshake has completed.
  ui_redirect: String,
  /// A secret to use when creating JWT tokens for our cookie.
  session_secret: String,
  /// The name of our cookie which will house our JWT token.
  session_cookie: String,
}

/// The web worker configuration.
#[derive(Deserialize)]
pub struct Configuration {
  /// General web configuration.
  pub(self) web: WebConfiguration,
  /// General redis configuration.
  pub(self) redis: crate::config::RedisConfiguration,
  /// General mongo configuration.
  pub(self) mongo: crate::config::MongoConfiguration,
  /// General mongo configuration.
  pub(self) google: crate::config::GoogleConfiguration,
  /// General mongo configuration.
  pub(self) registrar: crate::config::RegistrarConfiguration,
}

/// The json schema of our response sent from the heartbeat api.
#[derive(Serialize, Debug)]
struct HeartbeatPayload {
  /// The SHA of our application.
  version: String,
  /// The current timstamp.
  timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for HeartbeatPayload {
  fn default() -> Self {
    HeartbeatPayload {
      // TODO: pulling in compile-time environment varibles this "deep" in the code is
      // not ideal. It would be better for that to be handled by the consumer, but for
      // now this is quick-and-dirty.
      version: option_env!("BEETLE_VERSION").unwrap_or("dev").into(),
      timestamp: chrono::Utc::now(),
    }
  }
}

/// An api route to verify uptime/availability.
async fn heartbeat<T>(_request: tide::Request<T>) -> tide::Result {
  tide::Body::from_json(&HeartbeatPayload::default()).map(|body| tide::Response::builder(200).body(body).build())
}

/// The 404 handler.
async fn missing(_request: tide::Request<worker::Worker>) -> tide::Result {
  Ok(tide::Response::builder(404).build())
}

/// Returns a `tide::Server` that has been associated with our api routes.
pub fn new(worker: worker::Worker) -> tide::Server<worker::Worker> {
  let mut app = tide::with_state(worker);

  app.at("/auth/g/redirect").get(auth::google::redirect);
  app.at("/auth/g/complete").get(auth::google::complete);

  app.at("/auth/identify").get(auth::identify);
  app.at("/auth/logout").get(auth::logout);

  app.at("/devices/register").post(devices::register);
  app.at("/devices/unregister").post(devices::unregister);
  app.at("/device-info").get(devices::info);
  app.at("/device-authority").get(devices::authority);

  // Note: this api route has become the catch-all entrypoint for an interface into manipulating
  // devices. It is how we schedule lighting, messaging and calendar based modifications to
  // devices.
  app.at("/device-queue").post(jobs::queue);
  // TODO: this was added to support file uploads since we cannot use the multipart http body
  // format that elm would otherwise like to use.
  //
  // this should deprecate the non-scoped route, or make file uploading better.
  app.at("/device-queue/:device_id").post(jobs::queue);

  app.at("/jobs").get(jobs::find);
  app.at("/device-schedules").get(schedules::find);

  app.at("/status").get(heartbeat);
  app.at("/*").all(missing);
  app.at("/").all(missing);

  app
}

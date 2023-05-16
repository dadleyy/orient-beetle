use serde::{Deserialize, Serialize};

/// The api for authenticating.
mod auth;

/// The api for claiming access to devices.
mod claims;

/// The device api routes modules.
mod devices;

/// The main worker module.
mod worker;

pub use worker::Worker;

/// These configuration definitions makes it easy for the web binary to
/// deserialize a configuration file (e.g toml) and have everything ready
/// for the server to run.

/// Web configuration.
#[derive(Deserialize, Clone)]
pub struct WebConfiguration {
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
  /// General auth0 configuration.
  pub(self) auth0: crate::config::Auth0Configuration,
  /// General mongo configuration.
  pub(self) mongo: crate::config::MongoConfiguration,
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
  Ok(
    tide::Response::builder(200)
      .body(tide::Body::from_json(&HeartbeatPayload::default())?)
      .build(),
  )
}

/// The 404 handler.
async fn missing(_request: tide::Request<worker::Worker>) -> tide::Result {
  log::debug!("not-found");
  Ok(tide::Response::builder(404).build())
}

/// Returns a `tide::Server` that has been associated with our api routes.
pub fn new(worker: worker::Worker) -> tide::Server<worker::Worker> {
  let mut app = tide::with_state(worker);

  app.at("/auth/redirect").get(auth::redirect);
  app.at("/auth/complete").get(auth::complete);
  app.at("/auth/identify").get(auth::identify);
  app.at("/auth/logout").get(auth::logout);

  app.at("/devices/register").post(devices::register);
  app.at("/devices/unregister").post(devices::unregister);
  app.at("/device-info").get(devices::info);
  app.at("/device-message").post(devices::message);
  app.at("/device-queue").post(devices::queue);

  app.at("/status").get(heartbeat);
  app.at("/*").all(missing);
  app.at("/").all(missing);

  app
}

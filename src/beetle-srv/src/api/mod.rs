use serde::{Deserialize, Serialize};

mod auth;
mod claims;
mod devices;
mod messages;
mod worker;

pub use worker::Worker;

/// These configuration definitions makes it easy for the web binary to
/// deserialize a configuration file (e.g toml) and have everything ready
/// for the server to run.

#[derive(Deserialize, Clone)]
pub struct WebConfiguration {
  ui_redirect: String,
  session_secret: String,
  session_cookie: String,
}

#[derive(Deserialize)]
pub struct Configuration {
  pub(self) web: WebConfiguration,
  pub(self) redis: crate::config::RedisConfiguration,
  pub(self) auth0: crate::config::Auth0Configuration,
  pub(self) mongo: crate::config::MongoConfiguration,
}

#[derive(Serialize, Debug)]
struct HeartbeatPayload {
  version: String,
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

async fn heartbeat<T>(_request: tide::Request<T>) -> tide::Result {
  Ok(
    tide::Response::builder(200)
      .body(tide::Body::from_json(&HeartbeatPayload::default())?)
      .build(),
  )
}

async fn missing(_request: tide::Request<worker::Worker>) -> tide::Result {
  log::debug!("not-found");
  Ok(tide::Response::builder(404).build())
}

pub fn new(worker: worker::Worker) -> tide::Server<worker::Worker> {
  let mut app = tide::with_state(worker);

  app.at("/send-device-message").post(messages::send_message);

  app.at("/auth/redirect").get(auth::redirect);
  app.at("/auth/complete").get(auth::complete);
  app.at("/auth/identify").get(auth::identify);

  app.at("/devices/register").post(devices::register);
  app.at("/devices/unregister").post(devices::unregister);
  app.at("/device-info").get(devices::info);
  app.at("/device-message").post(devices::message);

  app.at("/status").get(heartbeat);
  app.at("/*").all(missing);
  app.at("/").all(missing);

  app
}

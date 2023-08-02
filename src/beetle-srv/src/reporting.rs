//! This module contains the code related to useful runtime metrics that can be reported to third
//! party analyics platforms like newrelic.

use serde::Serialize;
use std::io;

/// This is the enumerated type that holds all things sent to our third party analytics platform
/// for platform health monitoring.
#[derive(Serialize, Debug)]
pub enum Event {
  /// The length of our job queue.
  JobQueueLengthSample {
    /// The length of our job queue.
    queue_length: u16,
  },
}

/// The worker that holds a configuration, and works off sending events to some endpoint from a
/// channel.
pub struct Worker {
  /// The configuration.
  config: crate::config::RegistrarAnalyticsConfiguration,

  /// The receiving side of our event queue.
  receiver: async_std::channel::Receiver<Event>,
}

/// The container of our analytic event.
#[derive(Debug, serde::Serialize)]
struct QueueDiagnosticSample {
  /// The length of our job queue.
  #[serde(rename = "queue_length")]
  queue_length: u16,

  /// The constant event name for this payload.
  #[serde(rename = "eventType")]
  event_type: &'static str,
}

impl Worker {
  /// The constructor.
  pub fn new(config: crate::config::RegistrarAnalyticsConfiguration) -> (Self, async_std::channel::Sender<Event>) {
    let (sender, receiver) = async_std::channel::unbounded();
    (Self { config, receiver }, sender)
  }

  /// The main, async entrypoint for our reporting worker. It is meant to be pretty basic, we're
  /// just taking the next item from a channel and doing something with it based on our
  /// configuration.
  pub async fn work(self) -> io::Result<()> {
    loop {
      if self.receiver.is_closed() {
        return Err(io::Error::new(io::ErrorKind::Other, "reporting queue closed"));
      }

      let event = self.receiver.recv().await.map_err(|error| {
        io::Error::new(
          io::ErrorKind::Other,
          format!("failed taking next reporting event - {error}"),
        )
      })?;
      log::info!("reporting has next event to send along - {event:?}");

      match (event, &self.config) {
        (
          Event::JobQueueLengthSample { queue_length },
          crate::config::RegistrarAnalyticsConfiguration::NewRelic { account_id, api_key },
        ) => {
          let sample = QueueDiagnosticSample {
            queue_length,
            event_type: "registrarJobQueueSample",
          };

          let response = surf::post(format!(
            "https://insights-collector.newrelic.com/v1/accounts/{account_id}/event"
          ))
          .header("Accept", "*/*")
          .header("Api-Key", api_key)
          .body_json(&sample)
          .map_err(|error| {
            io::Error::new(
              io::ErrorKind::Other,
              format!("Unable to serialize queue sample - {error}"),
            )
          })?
          .await;

          log::info!(
            "analytics sample '{sample:?}' sent - {:?}",
            response.map(|success| success.status())
          );
        }
      }
    }
  }
}

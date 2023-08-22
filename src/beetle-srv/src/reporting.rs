//! This module contains the code related to useful runtime metrics that can be reported to third
//! party analyics platforms like newrelic.

use serde::Serialize;
use std::io;

/// The event name in newrelic for our queue health samples.
const QUEUE_DIAGNOSTIC_SAMPLE_EVENT_NAME: &str = "registrarJobQueueSample";

/// The event name in newrelic for our job processed counts.
const JOB_BATCH_PROCESSED_EVENT_NAME: &str = "registrarJobBatchProcessed";

/// The event name in newrelic for our device diganostic handled.
const DEVICE_DIAGNOSTIC_INGESTION_BATCH: &str = "deviceDiagnosticIngestionBatch";

/// This is the enumerated type that holds all things sent to our third party analytics platform
/// for platform health monitoring.
#[derive(Serialize, Debug)]
pub enum Event {
  /// This event is submitted by the registrar whenever it hears from devices on the incoming
  /// queue.
  DeviceDiganosticBatchIngested {
    /// The amount of devices processed.
    device_count: u16,
  },
  /// When the registrar finishes working jobs up to the max batch amount, this event is sent.
  JobBatchProcessed {
    /// The amount of jobs processed.
    job_count: u16,
  },
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
struct DeviceDiganosticBatchIngested {
  /// The amount of jobs processed.
  #[serde(rename = "device_count")]
  device_count: u16,

  /// The constant event name for this payload.
  #[serde(rename = "eventType")]
  event_type: &'static str,
}

/// The container of our analytic event.
#[derive(Debug, serde::Serialize)]
struct BatchProcessedSample {
  /// The amount of jobs processed.
  #[serde(rename = "job_count")]
  job_count: u16,

  /// The constant event name for this payload.
  #[serde(rename = "eventType")]
  event_type: &'static str,
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

      log::trace!("reporting has next event to send along - {event:?}");

      let result = match (event, &self.config) {
        (
          Event::DeviceDiganosticBatchIngested { device_count },
          crate::config::RegistrarAnalyticsConfiguration::NewRelic { account_id, api_key },
        ) => {
          self
            .send_newrelic(
              DeviceDiganosticBatchIngested {
                device_count,
                event_type: DEVICE_DIAGNOSTIC_INGESTION_BATCH,
              },
              account_id,
              api_key,
            )
            .await
        }
        (
          Event::JobBatchProcessed { job_count },
          crate::config::RegistrarAnalyticsConfiguration::NewRelic { account_id, api_key },
        ) => {
          self
            .send_newrelic(
              BatchProcessedSample {
                job_count,
                event_type: JOB_BATCH_PROCESSED_EVENT_NAME,
              },
              account_id,
              api_key,
            )
            .await
        }
        (
          Event::JobQueueLengthSample { queue_length },
          crate::config::RegistrarAnalyticsConfiguration::NewRelic { account_id, api_key },
        ) => {
          let sample = QueueDiagnosticSample {
            queue_length,
            event_type: QUEUE_DIAGNOSTIC_SAMPLE_EVENT_NAME,
          };

          self.send_newrelic(&sample, account_id, api_key).await
        }
      };

      if let Err(error) = result {
        log::error!("reporting worker unable to send event - {error}");
      }
    }
  }

  /// Sends events along to the newrelic api.
  async fn send_newrelic<T, S>(&self, data: T, account_id: S, api_key: S) -> io::Result<()>
  where
    T: serde::Serialize,
    S: AsRef<str>,
  {
    surf::post(format!(
      "https://insights-collector.newrelic.com/v1/accounts/{}/event",
      account_id.as_ref()
    ))
    .header("Accept", "*/*")
    .header("Api-Key", api_key.as_ref())
    .body_json(&data)
    .map_err(|error| {
      io::Error::new(
        io::ErrorKind::Other,
        format!("Unable to serialize queue sample - {error}"),
      )
    })?
    .await
    .map(|_| ())
    .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
  }
}

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::io;

/// The schema of the url query included in the redirect back from google oauth.
#[derive(Deserialize, Debug, Clone)]
pub struct CodeQuery {
  /// The code that will be exchanged for a token.
  pub code: String,
}

/// The schema of our userinfo json response.
#[derive(Deserialize, Debug)]
pub struct Userinfo {
  /// The id per google.
  #[allow(unused)]
  pub(crate) id: String,
  /// A url to a photo of the user.
  #[allow(unused)]
  pub(crate) picture: String,
  /// The users name.
  #[allow(unused)]
  pub(crate) name: String,
}

/// The schema of google's calendarlist api.
#[derive(Deserialize, Debug)]
pub struct CalendarList {
  /// A constant.
  #[allow(unused)]
  pub(crate) kind: String,

  /// To be determined.
  #[allow(unused)]
  pub(crate) etag: String,

  /// To be determined.
  pub(crate) items: Vec<CalendarListEntry>,
}

/// The schema used by event start/end values in the google api.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventListEntryTimeMarker {
  /// For whole-day events, only a `date` field is included.
  pub(crate) date: Option<String>,

  /// For normal events, the `date_time` includes time information.
  pub(crate) date_time: Option<String>,

  /// The timezone of an event.
  #[allow(unused)]
  pub(crate) time_zone: Option<String>,
}

/// The schema of google's event api items.
#[derive(Deserialize, Debug, Clone)]
pub struct EventListEntry {
  #[allow(unused, clippy::missing_docs_in_private_items)]
  pub(crate) id: String,
  #[allow(unused, clippy::missing_docs_in_private_items)]
  pub(crate) etag: String,
  #[allow(unused, clippy::missing_docs_in_private_items)]
  pub(crate) status: String,
  #[allow(clippy::missing_docs_in_private_items)]
  pub(crate) summary: String,
  #[allow(clippy::missing_docs_in_private_items)]
  pub(crate) start: EventListEntryTimeMarker,
  #[allow(clippy::missing_docs_in_private_items)]
  pub(crate) end: EventListEntryTimeMarker,
}

/// The schema of google's event list api.
#[derive(Deserialize, Debug, Clone)]
pub struct EventList {
  /// The list of items.
  items: Vec<EventListEntry>,
}

/// The schema of a single calendar.
#[derive(Deserialize, Debug, Clone)]
pub struct CalendarListEntry {
  #[allow(clippy::missing_docs_in_private_items)]
  id: String,
  #[allow(unused, clippy::missing_docs_in_private_items)]
  summary: Option<String>,
  #[allow(unused, clippy::missing_docs_in_private_items)]
  description: Option<String>,
  #[allow(clippy::missing_docs_in_private_items)]
  primary: Option<bool>,
}

/// The schema of google's successful code -> token exchange.
#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct TokenResponse {
  #[allow(clippy::missing_docs_in_private_items)]
  pub access_token: String,
  #[allow(clippy::missing_docs_in_private_items)]
  pub refresh_token: Option<String>,
  #[allow(clippy::missing_docs_in_private_items)]
  pub expires_in: u64,
}

/// The schema of a payload sent to google when exchanging a refresh token for a new access token.
#[derive(Serialize)]
pub struct TokenRefreshRequest {
  #[allow(clippy::missing_docs_in_private_items)]
  pub grant_type: &'static str,
  #[allow(clippy::missing_docs_in_private_items)]
  pub client_id: String,
  #[allow(clippy::missing_docs_in_private_items)]
  pub client_secret: String,
  #[allow(clippy::missing_docs_in_private_items)]
  pub refresh_token: String,
}

/// The schema of a payload sent to google when exchanging a code for a token.
#[derive(Serialize)]
pub struct TokenRequest {
  #[allow(clippy::missing_docs_in_private_items)]
  pub code: String,
  #[allow(clippy::missing_docs_in_private_items)]
  pub grant_type: &'static str,
  #[allow(clippy::missing_docs_in_private_items)]
  pub redirect_uri: String,
  #[allow(clippy::missing_docs_in_private_items)]
  pub client_id: String,
  #[allow(clippy::missing_docs_in_private_items)]
  pub client_secret: String,
}

/// Our type that wraps a token with a timestamp that holds when we received it.
#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct TokenHandle {
  /// The time we received this token.
  #[allow(unused)]
  pub(crate) created: chrono::DateTime<chrono::Utc>,
  /// The underlying token.
  pub(crate) token: TokenResponse,
}

/// An enumerated type meant to distinguish whole-day events from timed events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "beetle:kind", content = "beetle:content")]
pub enum ParsedEventTimeMarker {
  /// A whole-day event.
  Date(u32, u32, u32),
  /// An event with a specific time.
  DateTime(chrono::DateTime<chrono::offset::FixedOffset>),
}

/// The schema of an event that our application is concerned with.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParsedEvent {
  /// The id of this event.
  pub id: String,
  /// The text associated with this event.
  pub summary: String,
  /// The parsed, enumerated type holding our event start.
  pub start: ParsedEventTimeMarker,
  /// The parsed, enumerated type holding our event end.
  pub end: ParsedEventTimeMarker,
}

/// Normalizes events from their schema per google into the structure we will use in our
/// application.
pub fn parse_event(event: &EventListEntry) -> anyhow::Result<ParsedEvent> {
  let date_container = event.start.date.as_ref().zip(event.end.date.as_ref());
  let datetime_container = event.start.date_time.as_ref().zip(event.end.date_time.as_ref());

  match (date_container, datetime_container) {
    (Some((start, end)), None) => {
      log::trace!("found whole-day event {start} - {end}");
      let start = parse_event_date(start)?;
      let end = parse_event_date(end)?;

      Ok(ParsedEvent {
        id: event.id.clone(),
        summary: event.summary.clone(),
        start: ParsedEventTimeMarker::Date(start.0, start.1, start.2),
        end: ParsedEventTimeMarker::Date(end.0, end.1, end.2),
      })
    }
    (None, Some((start, end))) => {
      log::trace!("found timed event {start} - {end}");
      let start = chrono::DateTime::parse_from_rfc3339(start.as_str()).with_context(|| "invalid date")?;
      let end = chrono::DateTime::parse_from_rfc3339(end.as_str()).with_context(|| "invalid date")?;

      Ok(ParsedEvent {
        id: event.id.clone(),
        summary: event.summary.clone(),
        start: ParsedEventTimeMarker::DateTime(start),
        end: ParsedEventTimeMarker::DateTime(end),
      })
    }
    _ => Err(anyhow::Error::msg(format!("invalid date on source event - {event:?})"))),
  }
}

/// Will attempt to find the primary calendar associated with an access token for a given user.
pub async fn fetch_primary(handle: &TokenHandle) -> anyhow::Result<Option<CalendarListEntry>> {
  let mut res = surf::get("https://www.googleapis.com/calendar/v3/users/me/calendarList")
    .header("Authorization", format!("Bearer {}", handle.token.access_token))
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    .with_context(|| "cannot fetch")?;

  if res.status() != 200 {
    let body = res
      .body_string()
      .await
      .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
      .with_context(|| "unable to read failed google response body")?;

    log::warn!("bad primary calendar response from google - '{body}'");

    return Err(anyhow::Error::msg(format!(
      "bad status from primary calendar fetch attempt - '{}'",
      res.status()
    )));
  }

  let list = res
    .body_json::<CalendarList>()
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    .with_context(|| "cannot parse")?;

  log::debug!("found {} calendars", list.items.len());

  log::trace!("status: '{}'", res.status());
  log::trace!("list:   '{:?}'", list);

  Ok(list.items.iter().find(|e| matches!(e.primary, Some(true))).cloned())
}

/// Fetches calendar events associated with a token handle and calendar.
pub async fn fetch_events(handle: &TokenHandle, calendar: &CalendarListEntry) -> anyhow::Result<Vec<EventListEntry>> {
  log::debug!("fetching calendar '{}'", calendar.id);
  let mut uri = url::Url::parse(
    format!(
      " https://www.googleapis.com/calendar/v3/calendars/{}/events",
      calendar.id
    )
    .as_str(),
  )
  .with_context(|| "bad url")?;

  {
    let mut query = uri.query_pairs_mut();
    let now = chrono::Utc::now();
    let time_min = now.to_rfc3339();
    let time_max = std::ops::Add::add(now, chrono::Duration::days(1));

    query.append_pair("timeMin", time_min.as_str());
    query.append_pair("timeMax", time_max.to_rfc3339().as_str());
    query.append_pair("orderBy", "startTime");
    query.append_pair("singleEvents", "true");
  }

  let mut res = surf::get(&uri)
    .header("Authorization", format!("Bearer {}", handle.token.access_token))
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    .with_context(|| "cannot fetch")?;

  log::trace!("status: {}", res.status());
  let body_string = res
    .body_string()
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    .with_context(|| "no body")?;

  let events = serde_json::from_str::<EventList>(&body_string).with_context(|| "failed event list parse")?;

  log::debug!("found {} items in calendar", events.items.len());

  Ok(events.items)
}

/// Fetches user information from the google oauth api.
pub async fn fetch_user(handle: &TokenHandle) -> anyhow::Result<Userinfo> {
  let url = url::Url::parse("https://www.googleapis.com/oauth2/v1/userinfo").with_context(|| "invalid url")?;
  log::trace!("fetching profile '{url}'");

  let mut res = surf::get(&url)
    .header("Authorization", format!("Bearer {}", handle.token.access_token))
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    .with_context(|| "profile request failed")?;

  let userinfo = res
    .body_json::<Userinfo>()
    .await
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    .with_context(|| "body read failed")?;

  log::trace!("profile - '{}'", userinfo.id);

  Ok(userinfo)
}

/// A function used to print the primary calendar assocaited with a given access token to stdout.
#[allow(unused)]
pub async fn print_calendar(handle: &TokenHandle) -> anyhow::Result<()> {
  let primary = fetch_primary(handle).await?;
  let primary = primary.ok_or_else(|| anyhow::Error::msg("cannot find primary"))?;
  let events = fetch_events(handle, &primary).await?;

  for e in &events {
    let parsed = match parse_event(e) {
      Ok(e) => e,
      Err(e) => {
        log::warn!("unable to parse - {e}");
        continue;
      }
    };

    println!("-      {} ({})", parsed.summary, parsed.id);
    println!("start: {:?}", parsed.start);
    println!("  end: {:?}", parsed.end);
    println!("--------------");
  }

  Ok(())
}

/// Will attempt to parse the date format provided by google calendar events into a (YYYY, MM, DD)
/// tuple of u32 values. It is possible there is a better way to do this using chrono directly.
fn parse_event_date<S>(input: S) -> anyhow::Result<(u32, u32, u32)>
where
  S: AsRef<str>,
{
  let mut parsed = chrono::format::Parsed::new();
  chrono::format::parse(
    &mut parsed,
    input.as_ref(),
    vec![
      chrono::format::Item::Numeric(chrono::format::Numeric::Year, chrono::format::Pad::None),
      chrono::format::Item::Literal("-"),
      chrono::format::Item::Numeric(chrono::format::Numeric::Month, chrono::format::Pad::Zero),
      chrono::format::Item::Literal("-"),
      chrono::format::Item::Numeric(chrono::format::Numeric::Day, chrono::format::Pad::Zero),
    ]
    .iter(),
  )
  .with_context(|| "bad date")?;
  parsed
    .year
    .zip(parsed.month)
    .zip(parsed.day)
    .map(|((y, m), d)| (y as u32, m, d))
    .ok_or_else(|| anyhow::Error::msg("unable to find yyyy-mm-dd"))
}

#[cfg(test)]
mod tests {
  use super::parse_event_date;

  #[test]
  fn test_parse_event_date() {
    assert_eq!(parse_event_date("2023-01-01").expect("failed parse"), (2023, 1, 1));
  }
}

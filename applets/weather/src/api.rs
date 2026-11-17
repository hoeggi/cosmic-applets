// SPDX-License-Identifier: MPL-2.0

//! Open-Meteo HTTP client: forecast, air quality and place search. No API key is needed.
//!
//! The response types mirror the JSON; `model.rs` turns them into what the popup shows.
//! Every series value is optional, because Open-Meteo returns `null` where a model has no data.

use crate::config::{Location, Units};
use serde::Deserialize;
use std::fmt;
use std::time::Duration;

const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
const AIR_URL: &str = "https://air-quality-api.open-meteo.com/v1/air-quality";
const SEARCH_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";

/// Days in the forecast, today included.
pub const FORECAST_DAYS: u8 = 7;
/// 15-minute steps fetched for the precipitation alert (2 hours).
pub const MINUTELY_STEPS: usize = 8;
const SEARCH_RESULTS: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// No connection, DNS or TLS failure, timeout.
    Unreachable(String),
    /// HTTP 429: the daily or minutely request limit is used up.
    RateLimited,
    /// Open-Meteo rejected the request and said why.
    Rejected(String),
    Http(u16),
    /// The answer could not be parsed.
    Protocol(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(err) => write!(f, "unreachable: {err}"),
            Self::RateLimited => write!(f, "rate limited"),
            Self::Rejected(reason) => write!(f, "rejected: {reason}"),
            Self::Http(code) => write!(f, "HTTP {code}"),
            Self::Protocol(err) => write!(f, "unexpected answer: {err}"),
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Forecast {
    /// Offset of the location's time zone; all times in the response are local to it.
    pub utc_offset_seconds: i32,
    pub current: Current,
    pub minutely_15: Minutely,
    pub hourly: Hourly,
    pub daily: Daily,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Current {
    pub time: String,
    pub temperature_2m: Option<f64>,
    pub apparent_temperature: Option<f64>,
    pub weather_code: Option<u8>,
    pub is_day: Option<u8>,
    /// %.
    #[serde(default)]
    pub relative_humidity_2m: Option<f64>,
    /// km/h (mph).
    #[serde(default)]
    pub wind_speed_10m: Option<f64>,
    /// Degrees the wind comes from.
    #[serde(default)]
    pub wind_direction_10m: Option<f64>,
    #[serde(default)]
    pub wind_gusts_10m: Option<f64>,
    /// hPa, also with imperial units.
    #[serde(default)]
    pub pressure_msl: Option<f64>,
    /// Metres (feet).
    #[serde(default)]
    pub visibility: Option<f64>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Minutely {
    pub time: Vec<String>,
    /// Total in mm (inch): rain, showers and snow as water.
    pub precipitation: Vec<Option<f64>>,
    /// cm (inch).
    pub snowfall: Vec<Option<f64>>,
    pub weather_code: Vec<Option<u8>>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Hourly {
    pub time: Vec<String>,
    pub temperature_2m: Vec<Option<f64>>,
    pub precipitation: Vec<Option<f64>>,
    pub precipitation_probability: Vec<Option<f64>>,
    pub weather_code: Vec<Option<u8>>,
    pub is_day: Vec<Option<u8>>,
    pub wind_speed_10m: Vec<Option<f64>>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Daily {
    pub time: Vec<String>,
    pub weather_code: Vec<Option<u8>>,
    pub temperature_2m_max: Vec<Option<f64>>,
    pub temperature_2m_min: Vec<Option<f64>>,
    pub precipitation_sum: Vec<Option<f64>>,
    pub sunrise: Vec<Option<String>>,
    pub sunset: Vec<Option<String>>,
    pub uv_index_max: Vec<Option<f64>>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Air {
    pub current: AirCurrent,
}

/// Pollen is `null` outside Europe.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct AirCurrent {
    pub european_aqi: Option<f64>,
    pub pm2_5: Option<f64>,
    pub nitrogen_dioxide: Option<f64>,
    pub alder_pollen: Option<f64>,
    pub birch_pollen: Option<f64>,
    pub grass_pollen: Option<f64>,
    pub ragweed_pollen: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    /// Missing when nothing matched.
    #[serde(default)]
    results: Vec<Place>,
}

#[derive(Debug, Deserialize)]
struct Place {
    name: String,
    latitude: f64,
    longitude: f64,
    elevation: Option<f64>,
    country: Option<String>,
    admin1: Option<String>,
}

impl From<Place> for Location {
    fn from(place: Place) -> Self {
        let detail = [place.admin1, place.country]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        Location {
            name: place.name,
            detail,
            latitude: place.latitude,
            longitude: place.longitude,
            elevation: place.elevation,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    reason: String,
}

#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
}

impl Default for Client {
    fn default() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("cosmic-applet-weather/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        Self { http }
    }
}

impl Client {
    pub async fn forecast(&self, location: &Location, units: Units) -> Result<Forecast, ApiError> {
        let mut query = coordinates(location);
        query.extend([
            ("timezone", "auto".into()),
            ("forecast_days", FORECAST_DAYS.to_string()),
            ("forecast_minutely_15", MINUTELY_STEPS.to_string()),
            (
                "current",
                "temperature_2m,apparent_temperature,weather_code,is_day,relative_humidity_2m,\
                 wind_speed_10m,wind_direction_10m,wind_gusts_10m,pressure_msl,visibility"
                    .into(),
            ),
            ("minutely_15", "precipitation,snowfall,weather_code".into()),
            (
                "hourly",
                "temperature_2m,precipitation,precipitation_probability,weather_code,is_day,\
                 wind_speed_10m"
                    .into(),
            ),
            (
                "daily",
                "weather_code,temperature_2m_max,temperature_2m_min,precipitation_sum,sunrise,\
                 sunset,uv_index_max"
                    .into(),
            ),
        ]);
        if units == Units::Imperial {
            query.extend([
                ("temperature_unit", "fahrenheit".into()),
                ("wind_speed_unit", "mph".into()),
                ("precipitation_unit", "inch".into()),
            ]);
        }
        self.get(FORECAST_URL, &query).await
    }

    pub async fn air(&self, location: &Location) -> Result<Air, ApiError> {
        let mut query = coordinates(location);
        query.extend([
            ("timezone", "auto".into()),
            (
                "current",
                "european_aqi,pm2_5,nitrogen_dioxide,alder_pollen,birch_pollen,grass_pollen,\
                 ragweed_pollen"
                    .into(),
            ),
        ]);
        self.get(AIR_URL, &query).await
    }

    /// Places matching `name`, with names in `language` (e.g. `de`).
    pub async fn search(&self, name: &str, language: &str) -> Result<Vec<Location>, ApiError> {
        let query = [
            ("name", name.trim().to_owned()),
            ("count", SEARCH_RESULTS.to_string()),
            ("language", language.to_owned()),
            ("format", "json".into()),
        ];
        let response: SearchResponse = self.get(SEARCH_URL, &query).await?;
        Ok(response.results.into_iter().map(Location::from).collect())
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, String)],
    ) -> Result<T, ApiError> {
        let response = self
            .http
            .get(url)
            .query(query)
            .send()
            .await
            .map_err(|err| ApiError::Unreachable(error_chain(&err)))?;
        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(|err| ApiError::Unreachable(error_chain(&err)))?;
        parse(status.as_u16(), &body)
    }
}

fn coordinates(location: &Location) -> Vec<(&'static str, String)> {
    vec![
        ("latitude", format!("{:.4}", location.latitude)),
        ("longitude", format!("{:.4}", location.longitude)),
    ]
}

fn parse<T: serde::de::DeserializeOwned>(status: u16, body: &[u8]) -> Result<T, ApiError> {
    match status {
        200 => serde_json::from_slice(body).map_err(|err| ApiError::Protocol(err.to_string())),
        429 => Err(ApiError::RateLimited),
        _ => Err(serde_json::from_slice::<ErrorBody>(body)
            .map(|e| ApiError::Rejected(e.reason))
            .unwrap_or(ApiError::Http(status))),
    }
}

/// reqwest's top-level message hides the cause (e.g. DNS or certificate errors).
fn error_chain(err: &dyn std::error::Error) -> String {
    let mut message = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        message = format!("{message}: {cause}");
        source = cause.source();
    }
    message
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn forecast_fixture() -> Forecast {
        parse(200, include_bytes!("../tests/fixtures/forecast.json")).unwrap()
    }

    /// Talks to the real Open-Meteo API (needs network):
    /// `cargo test -p cosmic-applet-weather -- --ignored --nocapture live_api`
    #[tokio::test]
    #[ignore = "needs network access to Open-Meteo"]
    async fn live_api() {
        let client = Client::default();
        let places = client.search("Innsbruck", "de").await.unwrap();
        let place = places.first().expect("Innsbruck is found").clone();
        println!("place: {place:?}");

        for units in [Units::Metric, Units::Imperial] {
            let forecast = client.forecast(&place, units).await.unwrap();
            assert_eq!(forecast.minutely_15.time.len(), MINUTELY_STEPS);
            assert_eq!(forecast.daily.time.len(), usize::from(FORECAST_DAYS));
            let current = &forecast.current;
            println!("{units:?} details: {current:?}");
            assert!(current.relative_humidity_2m.is_some() && current.pressure_msl.is_some());
            assert!(current.wind_speed_10m.is_some() && current.visibility.is_some());
            let now = crate::model::local_now(forecast.utc_offset_seconds);
            let timeline = crate::model::timeline(&forecast, now).expect("hourly data covers now");
            println!(
                "{units:?}: {:?} now, columns {:?}",
                forecast.current.temperature_2m,
                timeline
                    .columns
                    .iter()
                    .map(|c| c.temperature)
                    .collect::<Vec<_>>()
            );
            println!(
                "alert: {:?}",
                crate::model::alert(&forecast.minutely_15, now, units)
            );
            assert_eq!(
                crate::model::days(&forecast, now).len(),
                usize::from(FORECAST_DAYS)
            );
        }
        let air = client.air(&place).await.unwrap();
        println!("air: {:?}", air.current);
        assert!(air.current.european_aqi.is_some());
    }

    #[test]
    fn parses_forecast() {
        let forecast = forecast_fixture();
        assert_eq!(forecast.utc_offset_seconds, 3600);
        assert_eq!(forecast.current.weather_code, Some(3));
        assert_eq!(forecast.minutely_15.time.len(), MINUTELY_STEPS);
        assert_eq!(forecast.hourly.time.len(), 24 * usize::from(FORECAST_DAYS));
        assert_eq!(forecast.daily.time.len(), usize::from(FORECAST_DAYS));
    }

    #[test]
    fn parses_air_without_pollen() {
        let body = br#"{"current":{"time":"2026-11-17T12:00","european_aqi":37,"pm2_5":13.4,
            "nitrogen_dioxide":15.1,"alder_pollen":null,"birch_pollen":null,"grass_pollen":null,
            "ragweed_pollen":null}}"#;
        let air: Air = parse(200, body).unwrap();
        assert_eq!(air.current.european_aqi, Some(37.0));
        assert_eq!(air.current.grass_pollen, None);
    }

    #[test]
    fn parses_search_results() {
        let body = br#"{"results":[{"id":1,"name":"Innsbruck","latitude":47.26,"longitude":11.39,
            "elevation":570.0,"country":"Austria","admin1":"Tirol"}]}"#;
        let response: SearchResponse = parse(200, body).unwrap();
        let places: Vec<Location> = response.results.into_iter().map(Location::from).collect();
        assert_eq!(places[0].name, "Innsbruck");
        assert_eq!(places[0].detail, "Tirol, Austria");
        let none: SearchResponse = parse(200, br#"{"generationtime_ms":0.1}"#).unwrap();
        assert!(none.results.is_empty());
    }

    #[test]
    fn maps_errors() {
        let rejected = parse::<Air>(
            400,
            br#"{"error":true,"reason":"Latitude must be in range"}"#,
        );
        assert_eq!(
            rejected,
            Err(ApiError::Rejected("Latitude must be in range".into()))
        );
        assert_eq!(parse::<Air>(429, b"{}"), Err(ApiError::RateLimited));
        assert_eq!(parse::<Air>(502, b"<html>"), Err(ApiError::Http(502)));
        assert!(matches!(
            parse::<Air>(200, b"{}"),
            Err(ApiError::Protocol(_))
        ));
    }
}

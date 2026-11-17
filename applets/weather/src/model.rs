// SPDX-License-Identifier: MPL-2.0

//! Turns Open-Meteo responses into what the popup and the panel show.
//!
//! All times are local to the location (Open-Meteo is asked for `timezone=auto`) and
//! handled as naive date-times. Precipitation values are sums over the *preceding*
//! period: a 15-minute value at 19:45 is the rain between 19:30 and 19:45, an hourly
//! value at 20:00 the rain between 19:00 and 20:00.

use crate::api::{AirCurrent, Forecast, Minutely};
use crate::config::Units;
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};

/// Columns of the 24-hour table, one every [`STEP`] hours. The chart shares this grid.
pub const COLUMNS: usize = 8;
pub const STEP: usize = 3;
/// Hours the chart spans: one column width beyond the last column's centre.
pub const SPAN: usize = COLUMNS * STEP;
/// Rain chances below this are left out of the table.
pub const SHOW_PROBABILITY_FROM: f64 = 20.0;

pub fn parse_time(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").ok()
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// The current time at the forecast's location.
pub fn local_now(utc_offset_seconds: i32) -> NaiveDateTime {
    chrono::Utc::now().naive_utc() + Duration::seconds(i64::from(utc_offset_seconds))
}

// ---------------------------------------------------------------- weather codes

/// Symbolic icon for a WMO weather code. The theme has night variants for clear skies only.
pub fn icon(code: u8, is_day: bool) -> &'static str {
    match (code, is_day) {
        (0 | 1, true) => "weather-clear-symbolic",
        (0 | 1, false) => "weather-clear-night-symbolic",
        (2, true) => "weather-few-clouds-symbolic",
        (2, false) => "weather-few-clouds-night-symbolic",
        (3, _) => "weather-overcast-symbolic",
        (45 | 48, _) => "weather-fog-symbolic",
        (51 | 80, _) => "weather-showers-scattered-symbolic",
        (53..=57 | 61..=67 | 81 | 82, _) => "weather-showers-symbolic",
        (71..=77 | 85 | 86, _) => "weather-snow-symbolic",
        (95..=99, _) => "weather-storm-symbolic",
        _ => "weather-severe-alert-symbolic",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precip {
    Rain,
    Snow,
    /// Freezing rain or drizzle: black ice.
    FreezingRain,
    Thunderstorm,
    /// Thunderstorm with hail.
    Hail,
}

impl Precip {
    /// The kind for a wet step; `snowfall` decides when the code itself is a dry one.
    fn from_code(code: u8, snowfall: f64) -> Self {
        match code {
            56 | 57 | 66 | 67 => Self::FreezingRain,
            71..=77 | 85 | 86 => Self::Snow,
            95 => Self::Thunderstorm,
            96 | 99 => Self::Hail,
            51..=55 | 61..=65 | 80..=82 => Self::Rain,
            _ if snowfall > 0.0 => Self::Snow,
            _ => Self::Rain,
        }
    }

    /// Identifier for the `precip` message in the ftl file.
    pub fn key(self) -> &'static str {
        match self {
            Self::Rain => "rain",
            Self::Snow => "snow",
            Self::FreezingRain => "freezing",
            Self::Thunderstorm => "thunderstorm",
            Self::Hail => "hail",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Rain | Self::FreezingRain => "weather-showers-symbolic",
            Self::Snow => "weather-snow-symbolic",
            Self::Thunderstorm | Self::Hail => "weather-storm-symbolic",
        }
    }

    /// Worth the warning colour rather than the accent.
    pub fn is_severe(self) -> bool {
        matches!(self, Self::FreezingRain | Self::Hail)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Intensity {
    Light,
    Moderate,
    Heavy,
}

impl Intensity {
    /// From the heaviest 15-minute amount, in mm (or inch with imperial units).
    fn from_quarter_hour(amount: f64, units: Units) -> Self {
        let mm_per_hour = 4.0 * amount * if units == Units::Imperial { 25.4 } else { 1.0 };
        if mm_per_hour < 2.5 {
            Self::Light
        } else if mm_per_hour < 7.6 {
            Self::Moderate
        } else {
            Self::Heavy
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Moderate => "moderate",
            Self::Heavy => "heavy",
        }
    }
}

// ---------------------------------------------------------------- precipitation alert

#[derive(Debug, Clone, PartialEq)]
pub enum When {
    /// Starts at this time.
    Starts(NaiveDateTime),
    /// Falling now and stops at this time.
    Until(NaiveDateTime),
    /// Falling now and through the whole 2-hour window.
    Ongoing,
}

/// Rain, snow etc. within the next 2 hours, from the 15-minute forecast.
#[derive(Debug, Clone, PartialEq)]
pub struct Alert {
    pub kind: Precip,
    pub intensity: Intensity,
    pub when: When,
    /// Snow expected in the window, in cm (inch); 0 for other kinds.
    pub snow: f64,
}

pub fn alert(minutely: &Minutely, now: NaiveDateTime, units: Units) -> Option<Alert> {
    // (start, end, amount, snowfall, code) of each step that has not ended yet.
    let steps: Vec<_> = minutely
        .time
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let end = parse_time(t)?;
            let value = |series: &[Option<f64>]| series.get(i).copied().flatten().unwrap_or(0.0);
            let code = minutely.weather_code.get(i).copied().flatten().unwrap_or(0);
            Some((
                end - Duration::minutes(15),
                end,
                value(&minutely.precipitation),
                value(&minutely.snowfall),
                code,
            ))
        })
        .filter(|(_, end, ..)| *end > now)
        .collect();

    let first = steps.iter().position(|step| step.2 > 0.0)?;
    let (start, _, _, snowfall, code) = steps[first];
    let wet = &steps[first..];
    let kind = Precip::from_code(code, snowfall);
    let heaviest = wet.iter().map(|step| step.2).fold(0.0, f64::max);
    let when = if start > now {
        When::Starts(start)
    } else {
        match wet.iter().find(|step| step.2 <= 0.0) {
            Some(dry) => When::Until(dry.0),
            None => When::Ongoing,
        }
    };
    let snow = if kind == Precip::Snow {
        wet.iter().map(|step| step.3).sum()
    } else {
        0.0
    };
    Some(Alert {
        kind,
        intensity: Intensity::from_quarter_hour(heaviest, units),
        when,
        snow,
    })
}

// ---------------------------------------------------------------- 24-hour timeline

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// `None` for the first column, which is "Now".
    pub hour: Option<u32>,
    pub temperature: Option<f64>,
    pub code: u8,
    pub is_day: bool,
    /// Shown only from [`SHOW_PROBABILITY_FROM`].
    pub probability: Option<f64>,
}

/// The next 24 hours on a grid of [`COLUMNS`] columns, each [`STEP`] hours wide and centred on
/// its hour. Positions are in hours from the current hour, so column `j` is centred on `j * STEP`.
#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    /// Hourly temperatures from now; index = hours from now. Gaps are `None`.
    pub temperatures: Vec<Option<f64>>,
    /// (centre in hours from now, amount): each hour's rain, drawn over the hour it fell in.
    pub precipitation: Vec<(f64, f64)>,
    /// Night spans (start, end) in hours from now, clipped to the chart.
    pub nights: Vec<(f64, f64)>,
    pub columns: Vec<Column>,
}

pub fn timeline(forecast: &Forecast, now: NaiveDateTime) -> Option<Timeline> {
    let hourly = &forecast.hourly;
    let times: Vec<Option<NaiveDateTime>> = hourly.time.iter().map(|t| parse_time(t)).collect();
    // From date and hour: `now` usually has seconds and nanoseconds.
    let hour_now = now.date().and_hms_opt(now.hour(), 0, 0)?;
    let first = times.iter().position(|t| *t == Some(hour_now))?;
    let at = |series: &[Option<f64>], k: usize| series.get(first + k).copied().flatten();

    // One hour beyond the span on both ends, so the curve reaches the chart's edges.
    let temperatures = (0..=SPAN).map(|k| at(&hourly.temperature_2m, k)).collect();
    let precipitation = (1..=SPAN)
        .filter_map(|k| {
            at(&hourly.precipitation, k)
                .filter(|amount| *amount > 0.0)
                .map(|amount| (k as f64 - 0.5, amount))
        })
        .collect();
    let columns = (0..COLUMNS)
        .map(|j| {
            let k = j * STEP;
            Column {
                hour: (j > 0).then(|| (hour_now + Duration::hours(k as i64)).hour()),
                temperature: at(&hourly.temperature_2m, k),
                code: hourly
                    .weather_code
                    .get(first + k)
                    .copied()
                    .flatten()
                    .unwrap_or(0),
                is_day: hourly.is_day.get(first + k).copied().flatten() != Some(0),
                probability: at(&hourly.precipitation_probability, k)
                    .filter(|p| *p >= SHOW_PROBABILITY_FROM),
            }
        })
        .collect();
    Some(Timeline {
        temperatures,
        precipitation,
        nights: nights(forecast, hour_now),
        columns,
    })
}

/// Spans between sunset and the next sunrise, in hours from `origin`, clipped to the chart
/// (which reaches half a column before the first and after the last column centre).
fn nights(forecast: &Forecast, origin: NaiveDateTime) -> Vec<(f64, f64)> {
    let hours = |t: NaiveDateTime| (t - origin).num_minutes() as f64 / 60.0;
    let (left, right) = (-(STEP as f64) / 2.0, SPAN as f64 - STEP as f64 / 2.0);
    let parse = |series: &[Option<String>]| -> Vec<NaiveDateTime> {
        series
            .iter()
            .filter_map(|t| t.as_deref().and_then(parse_time))
            .collect()
    };
    let (rises, sets) = (
        parse(&forecast.daily.sunrise),
        parse(&forecast.daily.sunset),
    );
    // Darkness before the first sunrise, then from each sunset to the following sunrise.
    let mut spans = Vec::new();
    if let Some(rise) = rises.first() {
        spans.push((f64::MIN, hours(*rise)));
    }
    for set in &sets {
        let next_rise = rises
            .iter()
            .find(|rise| *rise > set)
            .map_or(f64::MAX, |r| hours(*r));
        spans.push((hours(*set), next_rise));
    }
    spans
        .into_iter()
        .map(|(a, b)| (a.max(left), b.min(right)))
        .filter(|(a, b)| a < b)
        .collect()
}

// ---------------------------------------------------------------- days

#[derive(Debug, Clone, PartialEq)]
pub struct Day {
    pub date: NaiveDate,
    pub code: u8,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub precipitation: Option<f64>,
    pub sunrise: Option<NaiveTime>,
    pub sunset: Option<NaiveTime>,
    pub uv_max: Option<f64>,
    /// The day in 3-hour steps; for today only the steps not over yet.
    pub hours: Vec<Hour>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hour {
    pub time: NaiveTime,
    pub temperature: Option<f64>,
    pub code: u8,
    pub is_day: bool,
    /// Falling during the 3 hours from `time`.
    pub precipitation: f64,
    pub wind: Option<f64>,
}

pub fn days(forecast: &Forecast, now: NaiveDateTime) -> Vec<Day> {
    let daily = &forecast.daily;
    let hourly = &forecast.hourly;
    let times: Vec<Option<NaiveDateTime>> = hourly.time.iter().map(|t| parse_time(t)).collect();
    let get = |series: &[Option<f64>], i: usize| series.get(i).copied().flatten();
    let time_of = |series: &[Option<String>], i: usize| {
        series
            .get(i)
            .cloned()
            .flatten()
            .as_deref()
            .and_then(parse_time)
            .map(|t| t.time())
    };

    daily
        .time
        .iter()
        .enumerate()
        .filter_map(|(i, date)| {
            let date = parse_date(date)?;
            let hours = (0..24)
                .step_by(STEP)
                .filter_map(|h| {
                    let start = date.and_hms_opt(h, 0, 0)?;
                    if start + Duration::hours(STEP as i64) <= now {
                        return None;
                    }
                    let idx = times.iter().position(|t| *t == Some(start))?;
                    Some(Hour {
                        time: start.time(),
                        temperature: get(&hourly.temperature_2m, idx),
                        code: hourly.weather_code.get(idx).copied().flatten().unwrap_or(0),
                        is_day: hourly.is_day.get(idx).copied().flatten() != Some(0),
                        precipitation: (1..=STEP)
                            .filter_map(|k| get(&hourly.precipitation, idx + k))
                            .sum(),
                        wind: get(&hourly.wind_speed_10m, idx),
                    })
                })
                .collect();
            Some(Day {
                date,
                code: daily.weather_code.get(i).copied().flatten().unwrap_or(0),
                min: get(&daily.temperature_2m_min, i),
                max: get(&daily.temperature_2m_max, i),
                precipitation: get(&daily.precipitation_sum, i),
                sunrise: time_of(&daily.sunrise, i),
                sunset: time_of(&daily.sunset, i),
                uv_max: get(&daily.uv_index_max, i),
                hours,
            })
        })
        .filter(|day| day.date >= now.date())
        .collect()
}

/// Lowest and highest temperature over all days, for the shared range-bar scale.
pub fn range(days: &[Day]) -> Option<(f64, f64)> {
    let lo = days.iter().filter_map(|d| d.min).reduce(f64::min)?;
    let hi = days.iter().filter_map(|d| d.max).reduce(f64::max)?;
    (hi > lo).then_some((lo, hi))
}

/// Weekday number for the `weekday` message, 1 = Monday.
pub fn weekday(date: NaiveDate) -> u32 {
    date.weekday().number_from_monday()
}

// ---------------------------------------------------------------- air and pollen

/// European AQI band 0 (good) to 5 (extremely poor).
pub fn aqi_band(aqi: f64) -> u8 {
    match aqi {
        a if a < 20.0 => 0,
        a if a < 40.0 => 1,
        a if a < 60.0 => 2,
        a if a < 80.0 => 3,
        a if a < 100.0 => 4,
        _ => 5,
    }
}

/// UV band 0 (low) to 4 (extreme).
pub fn uv_band(uv: f64) -> u8 {
    match uv {
        u if u < 3.0 => 0,
        u if u < 6.0 => 1,
        u if u < 8.0 => 2,
        u if u < 11.0 => 3,
        _ => 4,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pollen {
    Alder,
    Birch,
    Grass,
    Ragweed,
}

impl Pollen {
    pub const ALL: [Self; 4] = [Self::Grass, Self::Birch, Self::Alder, Self::Ragweed];

    pub fn key(self) -> &'static str {
        match self {
            Self::Alder => "alder",
            Self::Birch => "birch",
            Self::Grass => "grass",
            Self::Ragweed => "ragweed",
        }
    }

    pub fn value(self, air: &AirCurrent) -> Option<f64> {
        match self {
            Self::Alder => air.alder_pollen,
            Self::Birch => air.birch_pollen,
            Self::Grass => air.grass_pollen,
            Self::Ragweed => air.ragweed_pollen,
        }
    }

    /// Level 0 (none) to 3 (high) from grains/m³. Rough thresholds: grass and ragweed
    /// cause symptoms at far lower counts than tree pollen.
    pub fn level(self, grains: f64) -> u8 {
        let (low, high) = match self {
            Self::Alder | Self::Birch => (10.0, 100.0),
            Self::Grass => (5.0, 50.0),
            Self::Ragweed => (5.0, 20.0),
        };
        if grains < 1.0 {
            0
        } else if grains <= low {
            1
        } else if grains <= high {
            2
        } else {
            3
        }
    }
}

// ---------------------------------------------------------------- formatting

pub fn temperature(value: f64) -> String {
    // Avoid "-0°".
    let rounded = value.round();
    format!("{}°", if rounded == 0.0 { 0.0 } else { rounded })
}

pub fn temperature_precise(value: f64) -> String {
    let value = if value.abs() < 0.05 { 0.0 } else { value };
    format!("{value:.1}°")
}

pub fn precipitation(amount: f64, units: Units) -> String {
    match units {
        Units::Metric => format!("{amount:.1} mm"),
        Units::Imperial => format!("{amount:.2} in"),
    }
}

pub fn snow(amount: f64, units: Units) -> String {
    match units {
        Units::Metric => format!("{amount:.0} cm"),
        Units::Imperial => format!("{amount:.1} in"),
    }
}

pub fn wind(speed: f64, units: Units) -> String {
    match units {
        Units::Metric => format!("{speed:.0} km/h"),
        Units::Imperial => format!("{speed:.0} mph"),
    }
}

pub fn clock(time: NaiveTime) -> String {
    time.format("%H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::tests::forecast_fixture;

    fn at(s: &str) -> NaiveDateTime {
        parse_time(s).unwrap()
    }

    fn minutely(amounts: &[f64], codes: &[u8], snowfall: &[f64]) -> Minutely {
        let start = at("2026-11-17T18:30");
        Minutely {
            time: (0..amounts.len())
                .map(|k| {
                    (start + Duration::minutes(15 * k as i64))
                        .format("%Y-%m-%dT%H:%M")
                        .to_string()
                })
                .collect(),
            precipitation: amounts.iter().copied().map(Some).collect(),
            snowfall: snowfall.iter().copied().map(Some).collect(),
            weather_code: codes.iter().copied().map(Some).collect(),
        }
    }

    #[test]
    fn alert_names_the_start_of_the_step_that_gets_wet() {
        // The 19:15 value is the rain between 19:00 and 19:15.
        let m = minutely(&[0.0, 0.0, 0.0, 0.1, 0.2], &[3, 3, 3, 61, 61], &[0.0; 5]);
        let alert = alert(&m, at("2026-11-17T18:20"), Units::Metric).unwrap();
        assert_eq!(alert.kind, Precip::Rain);
        assert_eq!(alert.intensity, Intensity::Light);
        assert_eq!(alert.when, When::Starts(at("2026-11-17T19:00")));
    }

    #[test]
    fn alert_for_snow_sums_the_snowfall() {
        let m = minutely(
            &[0.0, 0.3, 0.3, 0.0],
            &[3, 73, 73, 3],
            &[0.0, 0.4, 0.5, 0.0],
        );
        let alert = alert(&m, at("2026-11-17T18:20"), Units::Metric).unwrap();
        assert_eq!(alert.kind, Precip::Snow);
        assert!((alert.snow - 0.9).abs() < 1e-9);
    }

    #[test]
    fn alert_while_it_is_falling() {
        let m = minutely(&[0.2, 0.2, 0.0, 0.0], &[61; 4], &[0.0; 4]);
        let now = at("2026-11-17T18:20");
        assert_eq!(
            alert(&m, now, Units::Metric).unwrap().when,
            When::Until(at("2026-11-17T18:45"))
        );
        let wet = minutely(&[0.8; 4], &[63; 4], &[0.0; 4]);
        let all = alert(&wet, now, Units::Metric).unwrap();
        assert_eq!(all.when, When::Ongoing);
        assert_eq!(all.intensity, Intensity::Moderate);
    }

    #[test]
    fn alert_kinds_and_none_when_dry() {
        let kind = |code: u8| {
            let m = minutely(&[0.0, 0.1], &[3, code], &[0.0; 2]);
            alert(&m, at("2026-11-17T18:20"), Units::Metric)
                .unwrap()
                .kind
        };
        assert_eq!(kind(66), Precip::FreezingRain);
        assert_eq!(kind(95), Precip::Thunderstorm);
        assert_eq!(kind(99), Precip::Hail);
        assert_eq!(kind(81), Precip::Rain);
        let dry = minutely(&[0.0; 8], &[3; 8], &[0.0; 8]);
        assert_eq!(alert(&dry, at("2026-11-17T18:20"), Units::Metric), None);
        // Steps that are over do not count.
        let past = minutely(&[0.4, 0.0], &[61, 3], &[0.0; 2]);
        assert_eq!(alert(&past, at("2026-11-17T18:40"), Units::Metric), None);
    }

    #[test]
    fn intensity_in_inches() {
        assert_eq!(
            Intensity::from_quarter_hour(0.1, Units::Imperial),
            Intensity::Heavy
        );
        assert_eq!(
            Intensity::from_quarter_hour(0.1, Units::Metric),
            Intensity::Light
        );
    }

    #[test]
    fn timeline_columns_sit_every_three_hours_from_now() {
        let f = forecast_fixture();
        let t = timeline(&f, at("2026-11-17T18:40")).unwrap();
        assert_eq!(t.temperatures.len(), SPAN + 1);
        assert_eq!(t.temperatures[0], f.hourly.temperature_2m[18]);
        let hours: Vec<_> = t.columns.iter().map(|c| c.hour).collect();
        assert_eq!(
            hours,
            [
                None,
                Some(21),
                Some(0),
                Some(3),
                Some(6),
                Some(9),
                Some(12),
                Some(15)
            ]
        );
        assert_eq!(t.columns[1].temperature, f.hourly.temperature_2m[21]);
        // Only notable rain chances are shown.
        assert!(
            t.columns
                .iter()
                .all(|c| c.probability.is_none_or(|p| p >= 20.0))
        );
        // A real clock reading has seconds and nanoseconds.
        let precise = at("2026-11-17T18:40") + Duration::nanoseconds(12_345_678_901);
        assert_eq!(timeline(&f, precise), Some(t.clone()));
        // The rain of the hour ending at 20:00 is drawn over 19:00–20:00.
        assert_eq!(t.precipitation.first(), Some(&(1.5, 0.1)));
    }

    #[test]
    fn nights_cover_the_dark_hours() {
        let t = timeline(&forecast_fixture(), at("2026-11-17T18:40")).unwrap();
        // Sunset 16:13 today is before the chart; dark until sunrise 07:04 tomorrow, then dark
        // again from 16:13 to the chart's right edge (16:30).
        let night = t.nights[0];
        assert_eq!(night.0, -1.5);
        assert!((night.1 - (13.0 + 4.0 / 60.0)).abs() < 1e-9);
        assert_eq!(t.nights.len(), 2);
        assert!((t.nights[1].0 - (22.0 + 13.0 / 60.0)).abs() < 1e-9);
        assert_eq!(t.nights[1].1, 22.5);
    }

    #[test]
    fn days_in_three_hour_steps() {
        let f = forecast_fixture();
        let days = days(&f, at("2026-11-17T18:40"));
        assert_eq!(days.len(), 7);
        let today = &days[0];
        // Only the 18:00 and 21:00 steps of today are not over yet.
        let times: Vec<_> = today.hours.iter().map(|h| clock(h.time)).collect();
        assert_eq!(times, ["18:00", "21:00"]);
        assert_eq!(days[1].hours.len(), 8);
        assert_eq!(days[3].code, 73);
        assert_eq!(weekday(days[3].date), 5);
        assert_eq!(today.sunset, NaiveTime::from_hms_opt(16, 13, 0));
        let (lo, hi) = range(&days).unwrap();
        assert!(lo < hi);
        // A step sums the rain falling in its 3 hours (values at 19, 20 and 21:00).
        let expected: f64 = (19..=21).map(|h| f.hourly.precipitation[h].unwrap()).sum();
        assert!((today.hours[0].precipitation - expected).abs() < 1e-9);
    }

    #[test]
    fn bands_and_levels() {
        assert_eq!(aqi_band(46.0), 2);
        assert_eq!(aqi_band(150.0), 5);
        assert_eq!(uv_band(0.4), 0);
        assert_eq!(uv_band(7.0), 2);
        assert_eq!(Pollen::Grass.level(0.2), 0);
        assert_eq!(Pollen::Grass.level(30.0), 2);
        assert_eq!(Pollen::Birch.level(30.0), 2);
        assert_eq!(Pollen::Ragweed.level(30.0), 3);
    }

    #[test]
    fn formatting() {
        assert_eq!(temperature(-0.3), "0°");
        assert_eq!(temperature(14.6), "15°");
        assert_eq!(temperature_precise(-0.02), "0.0°");
        assert_eq!(precipitation(0.43, Units::Metric), "0.4 mm");
        assert_eq!(precipitation(0.017, Units::Imperial), "0.02 in");
        assert_eq!(wind(13.6, Units::Metric), "14 km/h");
        assert_eq!(icon(2, false), "weather-few-clouds-night-symbolic");
        assert_eq!(icon(73, true), "weather-snow-symbolic");
    }
}

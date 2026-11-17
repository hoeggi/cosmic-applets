# Changelog

All notable changes to `cosmic-applet-weather` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). Releases are tagged `weather-v<version>`.

## [0.1.0] - 2026-11-17

### Added
- Panel applet with the weather from Open-Meteo; no API key or account is needed.
- The panel shows the weather icon and the temperature, and optionally the start of rain or snow
  in the next 2 hours and today's high and low. It dims when the data is more than an hour old.
- The popup shows the current conditions with feels-like temperature, sunrise and sunset; tap
  them for humidity, wind and gusts, pressure, today's UV and visibility.
- An alert when rain, snow, freezing rain or a thunderstorm is due within the next 2 hours.
- The next 24 hours every 3 hours, with a chart of the temperature, hourly rain and night-time
  under it.
- 7 days with the rain amount and a low-to-high bar on one shared scale; tap a day for its hours
  in 3-hour steps with temperature, rain chance, rain amount and wind.
- Optional sections for air quality with UV, and for pollen (Europe only); both are off by
  default.
- Several saved places, found by name or entered as coordinates. Nothing is fetched until a place
  is chosen.
- Metric or imperial units, and a refresh every 10, 15, 30 or 60 minutes.

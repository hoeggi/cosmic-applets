# COSMIC Weather applet

Panel applet with the weather from [Open-Meteo](https://open-meteo.com/). No API key or account
is needed. The panel shows the current conditions as an icon and the temperature; the popup
focuses on what happens next:

- current conditions with feels-like temperature, sunrise and sunset
- an alert when rain, snow, freezing rain or a thunderstorm is due within the next 2 hours
  (from Open-Meteo's 15-minute forecast, which has real 15-minute data in Central Europe)
- the next 24 hours every 3 hours (time, icon, temperature), with a chart under it: the
  temperature curve, hourly rain and night shading, each point directly beneath its column
- 7 days with rain amount and a low-to-high bar on one shared scale; tap a day for its hours
  in 3-hour steps with temperature, rain chance, rain amount and wind
- optional sections for air quality with UV, and for pollen (Europe only); both are off by
  default, and turning either on adds one request to the air-quality API per refresh

Places are added in the settings by name (Open-Meteo's place search; postcodes only find the
city centre) or by coordinates. Several can be saved, and one is active. Nothing is fetched
until a place is chosen, and the location is never guessed from the IP address.

The forecast refreshes every 15 minutes by default (10, 15, 30 or 60). After an error it retries
within 5 minutes; when Open-Meteo's request limit is reached (HTTP 429) it waits an hour. Data
older than an hour is dimmed in the panel and marked in the popup.

Config lives in `~/.config/cosmic/at.hoeggi.CosmicAppletWeather/v1/`
(`locations`, `active`, `units`, `panel_temperature`, `panel_precipitation`, `panel_high_low`,
`show_air`, `show_pollen`, `refresh_mins`).

Weather data by [Open-Meteo.com](https://open-meteo.com/), licensed
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Open-Meteo's free API is for
non-commercial use.

## Development

Build, run and package from the repository root (see the [top-level README](../../README.md)):

    make run APPLET=weather

The tests use a synthetic forecast in `tests/fixtures/`. One opt-in test talks to the real API
(run inside `make shell`):

    cargo test -p cosmic-applet-weather -- --ignored --nocapture live_api

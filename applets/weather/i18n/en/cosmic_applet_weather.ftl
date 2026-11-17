app-title = Weather
app-comment = Current weather and forecast from Open-Meteo
app-keywords = Weather;Forecast;Temperature;Rain;Snow;Open-Meteo;

# Header
updated = { $minutes ->
    [0] Open-Meteo · updated just now
   *[other] Open-Meteo · updated { $minutes } min ago
}
updated-hours = Updated { $hours ->
    [one] 1 hour
   *[other] { $hours } hours
} ago
refresh = Refresh
settings = Settings

# Current conditions
feels-like = feels like { $temp }
condition = { $code ->
    [0] Clear sky
    [1] Mainly clear
    [2] Partly cloudy
    [3] Overcast
    [45] Fog
    [48] Freezing fog
    [51] Light drizzle
    [53] Drizzle
    [55] Heavy drizzle
    [56] Light freezing drizzle
    [57] Freezing drizzle
    [61] Light rain
    [63] Rain
    [65] Heavy rain
    [66] Light freezing rain
    [67] Freezing rain
    [71] Light snow
    [73] Snow
    [75] Heavy snow
    [77] Snow grains
    [80] Light showers
    [81] Showers
    [82] Heavy showers
    [85] Snow showers
    [86] Heavy snow showers
    [95] Thunderstorm
    [96] Thunderstorm with hail
    [99] Thunderstorm with heavy hail
   *[other] Unknown
}

# Details under the current conditions
show-details = Show details
hide-details = Hide details
humidity = Humidity
wind = Wind
gusts = Gusts
pressure = Pressure
uv-today = UV today
visibility = Visibility
uv-value = { $value } · { $band }
compass = { $point ->
    [0] N
    [1] NE
    [2] E
    [3] SE
    [4] S
    [5] SW
    [6] W
   *[7] NW
}

# Rain / snow alert for the next 2 hours
precip = { $kind ->
    [snow] { $intensity ->
        [light] Light snow
        [heavy] Heavy snow
       *[moderate] Snow
    }
    [freezing] Freezing rain
    [thunderstorm] Thunderstorm
    [hail] Thunderstorm with hail
   *[rain] { $intensity ->
        [light] Light rain
        [heavy] Heavy rain
       *[moderate] Rain
    }
}
alert-starts = { $what } from about { $time }
alert-until = { $what } until about { $time }
alert-ongoing = { $what } for the next 2 hours
alert-snow = { $alert } · about { $amount }
alert-ice = { $alert } · roads may ice

# Forecast
next-24-hours = Next 24 hours
now = Now
next-days = Next 7 days
today = Today
weekday = { $day ->
    [1] Mon
    [2] Tue
    [3] Wed
    [4] Thu
    [5] Fri
    [6] Sat
   *[7] Sun
}

# Air and pollen (optional sections)
air = Air
aqi = European AQI
aqi-band = { $band ->
    [0] Good
    [1] Fair
    [2] Moderate
    [3] Poor
    [4] Very poor
   *[5] Extremely poor
}
pm25 = PM2.5
no2 = NO₂
uv-max = UV index, max today
uv-band = { $band ->
    [0] Low
    [1] Moderate
    [2] High
    [3] Very high
   *[4] Extreme
}
pollen = Pollen
pollen-name = { $kind ->
    [grass] Grass
    [birch] Birch
    [alder] Alder
   *[ragweed] Ragweed
}
pollen-level = { $level ->
    [0] None
    [1] Low
    [2] Moderate
   *[3] High
}
pollen-unavailable = No pollen forecast for this place (Europe only).
air-error = Air quality unavailable: { $error }
attribution = Weather data by Open-Meteo.com (CC BY 4.0)

# States
no-location = No location yet
no-location-hint = Choose a place to see its weather. Nothing is fetched until you do.
choose-location = Choose location
loading = Fetching the forecast…
error-unreachable = Can't reach Open-Meteo: { $error }
error-rate-limited = Open-Meteo's request limit is reached. Next try in 1 hour; a longer refresh interval in Settings helps.
error-rejected = Open-Meteo rejected the request: { $error }
error-http = Open-Meteo answered with HTTP { $code }
error-protocol = Unexpected answer from Open-Meteo: { $error }
showing-old = The values below are from { $time }.
retry = Retry

# Settings
locations = Locations
add-location = Add location
elevation = { $metres } m
delete = Delete
units = Units
units-metric = °C · km/h · mm
units-imperial = °F · mph · in
panel = Panel
panel-temperature = Show temperature
panel-precipitation = Show rain or snow in the next 2 hours
panel-high-low = Show today's high and low
popup = Popup
show-air = Air quality and UV
show-air-hint = Adds a second request per refresh
show-pollen = Pollen
show-pollen-hint = Europe only
refresh-every = Refresh every
minutes = { $minutes } min

# Add location
search-placeholder = Place or district
search = Search
searching = Searching…
no-results = Nothing found for “{ $query }”
search-hint = Search by place or district name. Postcodes only find the city centre.
search-error = Search failed: { $error }
or-coordinates = Or enter coordinates
latitude = Latitude
longitude = Longitude
add = Add

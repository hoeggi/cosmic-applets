#!/usr/bin/env python3
"""Builds docs/weather-mockups/index.html: design proposals for a standalone Open-Meteo weather applet.

Built from docs/mockup-kit, so all mockups share one look. All values are invented, though shaped like real Open-Meteo responses.
Usage: python3 build.py index.html
"""
import os, sys

OUT = sys.argv[1]
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "mockup-kit"))
from kit import glyph, GLYPHS, i, ib, row, val, dot, header, DIV, toggle, seg, bar, tile, day, hour, CSS as KIT_CSS, WEATHER_CSS

GLYPHS.update({
    "pin": glyph('<path d="M8 1a4.5 4.5 0 0 0-4.5 4.5C3.5 9 8 15 8 15s4.5-6 4.5-9.5A4.5 4.5 0 0 0 8 1zm0 6.3a1.8 1.8 0 1 1 0-3.6 1.8 1.8 0 0 1 0 3.6z"/>'),
    "leaf": glyph('<path d="M14 2S6 1.5 3.5 6c-1.4 2.5-.6 5 .4 6.3L2 14.2l.8.8 1.9-1.9c1.3 1 3.8 1.8 6.3.4C15.5 11 14 2 14 2zm-8.5 9.8L11 6.3l-.7-.7-5.5 5.5z"/>'),
    "eye": glyph('<path d="M8 3.5C4.4 3.5 1.7 6 1 8c.7 2 3.4 4.5 7 4.5S14.3 10 15 8c-.7-2-3.4-4.5-7-4.5zm0 7.3a2.8 2.8 0 1 1 0-5.6 2.8 2.8 0 0 1 0 5.6zM8 6.6a1.4 1.4 0 1 0 0 2.8 1.4 1.4 0 0 0 0-2.8z"/>'),
    "gust": GLYPHS["wind"],
    "uv": GLYPHS["w-sun"],
    "aqi": glyph('<path d="M2 4h7.5a1.5 1.5 0 1 0-1.5-1.5H6.6A2.9 2.9 0 1 1 9.5 5.4H2zM2 7.4h11a1.5 1.5 0 1 1-1.5 1.5H10a2.9 2.9 0 1 0 2.9-2.9H2z"/><circle cx="4" cy="12" r="1.3"/><circle cx="8" cy="13" r="1"/><circle cx="11.5" cy="12" r="1.4"/>'),
})

LOC = "Vienna"
UPD = "Open-Meteo · updated 6 min ago"
ATTR = '<div class="attr">Weather data by Open-Meteo.com (CC BY 4.0)</div>'


def hero(temp="7.0°", text="Overcast", feels="feels like 5.4°", icon="w-cloud"):
    return (f'<div class="hero">{i(icon, "w48")}<div><div class="big">{temp}</div>'
            f'<div>{text}</div><div class="c">{feels}</div></div>'
            f'<div class="sun"><span>{i("w-rise")}07:04</span><span>{i("w-set")}16:13</span></div></div>')


def rainsoon(text):
    return f'<div class="soon">{i("w-rain")}<span>{text}</span></div>'


# ---- 24 h temperature line with precipitation bars (hourly.temperature_2m / precipitation) ----
HOURS = list(range(18, 24)) + list(range(0, 18))
TEMPS = [6.8, 6.5, 6.4, 6.2, 6.6, 7.1, 7.5, 7.9, 8.2, 8.5, 8.6, 8.8, 9.0, 9.4, 10.6, 12.1, 13.4, 14.2, 14.7, 14.5, 13.6, 12.3, 10.9, 9.8]
RAIN = [0, 0, .1, .4, .6, .3, .1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, .1, .2, .1, 0, 0]
PROB = [10, 20, 45, 70, 75, 60, 35, 15, 10, 8, 5, 5, 5, 5, 3, 3, 3, 5, 10, 25, 40, 30, 15, 10]


# Chart and table share one grid: COLS columns of EVERY hours each. Column j is centred on hour EVERY*j,
# and the curve is scaled so that hour lands exactly on the column centre.
COLS, EVERY, CW = 8, 3, 328
COLW = CW / COLS
def icon_at(k):
    """Weather icon for hour index k, derived from the same hourly data the chart draws."""
    if RAIN[k] > 0:
        return "w-rain"
    return "w-partly" if 8 <= HOURS[k] <= 15 and TEMPS[k] > 11 else "w-cloud"


def hx(k):
    """x of hour index k (0 = now): centre of column k / EVERY."""
    return COLW * (k / EVERY + .5)


def chart(w=CW, h=92):
    top, bot = 10, h - 6
    lo, hi = min(TEMPS) - 1, max(TEMPS) + 1
    xs = [hx(k) for k in range(len(TEMPS))]
    ys = [bot - 12 - (t - lo) / (hi - lo) * (bot - 12 - top) for t in TEMPS]
    pts = " ".join(f"{x:.1f},{y:.1f}" for x, y in zip(xs, ys))
    area = f"{xs[0]:.1f},{bot} {pts} {xs[-1]:.1f},{bot}"
    bw = COLW / EVERY * .64
    rain = "".join(
        f'<rect x="{x - bw / 2:.1f}" y="{bot - r * 30:.1f}" width="{bw:.1f}" height="{r * 30:.1f}" rx="1.5" class="pr"/>'
        for x, r in zip(xs, RAIN) if r)
    cols = range(0, COLS * EVERY, EVERY)
    guides = "".join(f'<line x1="{xs[k]:.1f}" x2="{xs[k]:.1f}" y1="{ys[k]:.1f}" y2="{bot}" class="guide"/>' for k in cols)
    dots = "".join(f'<circle cx="{xs[k]:.1f}" cy="{ys[k]:.1f}" r="{3.5 if k == 0 else 2.6}" class="{"now" if k == 0 else "dotc"}"/>' for k in cols)
    # night: before sunrise 07:04 (k=13.07) and after sunset 16:13 (k=22.2)
    night = (f'<rect x="0" y="0" width="{hx(13.07):.1f}" height="{bot}" class="night"/>'
             f'<rect x="{hx(22.2):.1f}" y="0" width="{w - hx(22.2):.1f}" height="{bot}" class="night"/>')
    return (f'<svg class="chart" viewBox="0 0 {w} {h}" width="{w}" height="{h}">{night}'
            f'<line x1="0" x2="{w}" y1="{bot}" y2="{bot}" class="base"/>{guides}{rain}'
            f'<polygon points="{area}" class="area"/><polyline points="{pts}" class="line"/>{dots}</svg>')


def hstrip():
    cells = []
    for j in range(COLS):
        k = j * EVERY
        p = f"{PROB[k]}%" if PROB[k] >= 20 else ""
        cells.append(("Now" if k == 0 else f"{HOURS[k]:02d}", icon_at(k), f"{TEMPS[k]:.0f}°", p))
    return f'<div class="hs" style="grid-template-columns:repeat({COLS},1fr)">' + "".join(
        f'<div><span class="v">{v}</span>{i(ic)}<span class="pp">{p}</span><span class="c">{t}</span></div>'
        for t, ic, v, p in cells) + "</div>"


# ---- Row-order options for the 24-hour block ----
def strip_row(kind):
    cells = []
    for j in range(COLS):
        k = j * EVERY
        if kind == "time":
            cells.append(f'<span class="c">{"Now" if k == 0 else f"{HOURS[k]:02d}"}</span>')
        elif kind == "icon":
            cells.append(i(icon_at(k)))
        elif kind == "temp":
            cells.append(f'<span class="v">{TEMPS[k]:.0f}°</span>')
        elif kind == "rain":
            cells.append(f'<span class="pp">{f"{PROB[k]}%" if PROB[k] >= 20 else ""}</span>')
    return (f'<div class="r24 r24-{kind}" style="grid-template-columns:repeat({COLS},1fr)">'
            + "".join(f"<div>{c}</div>" for c in cells) + "</div>")


def block24(order):
    parts = [f'<div class="chartw">{chart()}</div>' if kind == "chart" else strip_row(kind) for kind in order]
    return '<div class="gh">Next 24 hours</div>' + '<div class="b24">' + "".join(parts) + "</div>"


ORDER_OPTIONS = [
    ("o1", "1. Current", ["chart", "temp", "icon", "rain", "time"],
     "Chart, then temperature, icon, rain chance and time. The temperatures sit under the rain bars "
     "instead of near the curve, and the rain chance is two rows away from the bars it explains."),
    ("o2", "2. Grouped by what the chart shows (recommended)", ["icon", "temp", "chart", "rain", "time"],
     "Icon and temperature above the chart, close to the curve's dots; rain chance right below the rain "
     "bars; time last as the x-axis. Each row sits next to the part of the chart it belongs to."),
    ("o3", "3. Time first", ["time", "icon", "temp", "chart", "rain"],
     "Reads like a header row: time, icon, temperature, then the chart and the rain chance under its bars. "
     "Familiar from phone apps; the chart has no axis below it."),
    ("o4", "4. Table first, chart as summary", ["time", "icon", "temp", "rain", "chart"],
     "The classic hourly strip with the chart underneath as a visual summary. Simple to scan, but the "
     "numbers and the curve are farthest apart."),
    ("o5", "5. Minimal: no temperature row", ["icon", "chart", "rain", "time"],
     "Drops the temperature row; the curve carries it, and the exact value shows on hover. Shortest block, "
     "but you have to hover to read a number."),
]


def order_screen(key, title, order, note):
    return f'<section id="{key}"><h2>{title}</h2><div class="popup">{block24(order)}</div><p>{note}</p></section>'


def compact_day(name, icon, lo, hi, a, b, prob=""):
    p = f'<span class="pp">{prob}</span>' if prob else '<span class="pp"></span>'
    return (f'<div class="cday"><span class="dn">{name}</span>{i(icon)}{p}<span class="lo">{lo}</span>'
            f'<span class="rng"><b style="left:{a}%;right:{100 - b}%"></b></span><span class="v hi">{hi}</span></div>')


DAYS7 = [("Today", "w-rain", "4°", "15°", 30, 74, "100%"), ("Wed", "w-partly", "9°", "15°", 57, 74, "65%"),
         ("Thu", "w-cloud", "3°", "8°", 26, 46, ""), ("Fri", "w-snow", "-1°", "5°", 9, 34, "80%"),
         ("Sat", "w-cloud", "-2°", "3°", 4, 26, ""), ("Sun", "w-sun", "0°", "6°", 14, 37, ""),
         ("Mon", "w-partly", "2°", "9°", 20, 51, "")]

# ============================ PANEL ============================
PANEL_ITEMS = [
    (f'<span class="ap">{i("w-cloud")}<span>7°</span></span>', "P1 · Icon and temperature (default)"),
    (f'<span class="ap">{i("w-cloud")}</span>', "P2 · Icon only, temperature in the tooltip"),
    (f'<span class="ap">{i("w-cloud")}<span>7°</span><span class="sub">{i("w-rain")}20:00</span></span>', "P3 · Adds the next rain in the coming 2 h"),
    (f'<span class="ap on">{i("w-cloud")}<span>7°</span><span class="sub">14° / 4°</span></span>', "P4 · Adds today's high / low (popup open)"),
    (f'<span class="ap dimmed">{i("w-cloud")}<span>7°</span></span>', "Stale: no successful update for 1 h"),
    (f'<span class="ap">{i("w-cloud")}<span>7°</span><span class="badge"></span></span>', "Fetch failing, last value still shown"),
    (f'<span class="ap">{i("pin")}</span>', "First run: no location set"),
    (f'<span class="ap vert">{i("w-cloud")}<span>7°</span></span>', "Vertical panel: icon over temperature"),
]
PANEL = '<section class="wide"><h2>Panel button</h2><div class="bars bars4">' + "".join(
    f'<figure>{bar(content).replace("Sat Oct 31 21:14", "18:02")}<figcaption>{label}</figcaption></figure>'
    for content, label in PANEL_ITEMS) + '</div></section>'


def group(title, text):
    return f'<section class="wide grp"><h2>{title}</h2><p>{text}</p></section>'


def screen(key, title, note, body, data=""):
    d = f'<p class="data"><b>Open-Meteo:</b> {data}</p>' if data else ""
    return f'<section id="{key}"><h2>{title}</h2><div class="popup">{body}</div><p>{note}</p>{d}</section>'


# ============================ A · CLASSIC ============================
TILES_A = '<div class="tiles">' + "".join([
    tile("drop", "Humidity", "89%"), tile("wind", "Wind", "3km/h S"),
    tile("gust", "Gusts", "18km/h"), tile("gauge", "Pressure", "995hPa"),
    tile("uv", "UV today", "0.4 low"), tile("eye", "Visibility", "22km"),
]) + "</div>"

a_main = header(LOC, UPD, back=False, actions=("view-refresh", "emblem-system")) + DIV + hero() + TILES_A + DIV + \
    '<div class="gh">Next 7 days</div>' + \
    day("Today", "w-rain", "4°", "15°", 30, 74, "2.5mm", True) + \
    '<div class="hours">' + "".join([
        hour("19:00", "w-cloud", "6.5°", "", "4km/h"),
        hour("20:00", "w-rain", "6.4°", "0.1mm", "6km/h"),
        hour("21:00", "w-rain", "6.2°", "0.4mm", "9km/h"),
        hour("22:00", "w-rain", "6.6°", "0.6mm", "11km/h"),
        hour("23:00", "w-rain", "7.1°", "0.3mm", "10km/h"),
    ]) + '</div>' + \
    day("Wed", "w-partly", "9°", "15°", 57, 74, "0.4mm") + \
    day("Thu", "w-cloud", "3°", "8°", 26, 46) + \
    day("Fri", "w-snow", "-1°", "5°", 9, 34, "1.2mm") + \
    day("Sat", "w-cloud", "-2°", "3°", 4, 26) + ATTR

# ============================ B · TIMELINE ============================
def sec(n, body):
    return f'<div class="sec" data-n="{n}">{body}</div>'


def annotated(key, title, intro, body, notes):
    items = "".join(
        f'<li class="note" data-n="{n}"><span class="num">{n}</span><div><b>{t}</b><div>{d}</div>'
        + (f'<div class="src">{src}</div>' if src else "") + '</div></li>'
        for n, (t, d, src) in enumerate(notes, 1))
    return (f'<section id="{key}" class="anno"><h2>{title}</h2><p class="intro">{intro}</p>'
            f'<div class="annob"><div class="popup">{body}</div><ol class="notes">{items}</ol></div></section>')

b_day = header("Friday", f"{LOC} · Nov 20") + DIV + \
    f'<div class="hero">{i("w-snow", "w48")}<div><div class="big">5° <span class="lo2">/ -1°</span></div><div>Light snow</div>'\
    f'<div class="c">1.2mm over 4 hours · 80% chance</div></div></div>' + \
    '<div class="tiles">' + "".join([
        tile("wind", "Wind max", "22km/h W"), tile("gust", "Gusts max", "48km/h"),
        tile("w-sun", "Sunshine", "1h 10m"), tile("uv", "UV max", "0.8 low"),
        tile("w-rise", "Sunrise", "07:08"), tile("w-set", "Sunset", "16:10"),
    ]) + '</div>' + DIV + '<div class="hours tight">' + "".join([
        hour("00:00", "w-cloud", "1.2°", "", "12km/h"),
        hour("03:00", "w-snow", "0.4°", "0.2mm", "15km/h"),
        hour("06:00", "w-snow", "-0.6°", "0.5mm", "19km/h"),
        hour("09:00", "w-snow", "0.8°", "0.4mm", "22km/h"),
        hour("12:00", "w-cloud", "3.9°", "0.1mm", "20km/h"),
        hour("15:00", "w-partly", "4.8°", "", "17km/h"),
        hour("18:00", "w-cloud", "2.1°", "", "11km/h"),
        hour("21:00", "w-cloud", "0.3°", "", "9km/h"),
    ]) + '</div>' + ATTR

# ============================ C · AT A GLANCE + AIR ============================
def meter(label, value, pct, scale, cls=""):
    return (f'<div class="mt"><div class="mh"><span>{label}</span><span class="v">{value}</span></div>'
            f'<div class="mb {cls}"><i style="left:{pct}%"></i></div><div class="ms">{scale}</div></div>')


def pollen(name, level, n):
    pips = "".join(f'<i class="{"on" if k < n else ""}"></i>' for k in range(4))
    return f'<div class="pl"><span>{name}</span><span class="pips">{pips}</span><span class="c">{level}</span></div>'


c_main = header(LOC, UPD, back=False, actions=("view-refresh", "emblem-system")) + DIV + \
    f'<div class="glance">{i("w-cloud", "w48")}<div class="big">7°</div><div class="gl2"><div>Overcast</div>'\
    f'<div class="c">14° / 4° · feels 5°</div></div></div>' + \
    '<div class="chips">' + "".join(f'<span class="chip2">{i(a)}{b}</span>' for a, b in [
        ("w-rain", "Rain 20:00"), ("wind", "3 km/h S"), ("drop", "89%")]) + '</div>' + DIV + \
    '<div class="gh">Air</div><div class="pad">' + \
    meter("European AQI", "46 · Fair", 23, "<span>Good</span><span>Fair</span><span>Moderate</span><span>Poor</span><span>Very poor</span>", "aqi") + \
    '<div class="tiles in2">' + "".join([tile("aqi", "PM2.5", "19µg/m³"), tile("aqi", "NO₂", "36µg/m³")]) + '</div>' + \
    meter("UV index, max today", "0.4 · Low", 4, "<span>0</span><span>3</span><span>6</span><span>8</span><span>11+</span>", "uvm") + \
    '</div>' + DIV + '<div class="gh">Pollen</div><div class="pad pls">' + \
    pollen("Grass", "None", 0) + pollen("Birch", "None", 0) + pollen("Alder", "Low", 1) + pollen("Ragweed", "None", 0) + \
    '<div class="c">Pollen is forecast for Europe only, and rows can be hidden in Settings.</div></div>' + DIV + \
    '<div class="gh">Next 5 days</div><div class="cols">' + "".join(
        f'<div><span class="dn">{n}</span>{i(ic)}<span class="v">{hi}</span><span class="lo">{lo}</span></div>'
        for n, ic, lo, hi, *_ in DAYS7[:5]) + '</div>' + ATTR

# ============================ B · TIMELINE (defined after C: reuses its air/pollen helpers) ============================
def opt_gh(title):
    return f'<div class="gh ghx"><span>{title}</span><span class="tag">Optional · on in Settings</span></div>'


b_main = sec(1, header(LOC, UPD, back=False, actions=("view-refresh", "emblem-system"))) + DIV + \
    sec(2, hero()) + sec(3, rainsoon("Light rain from about 19:30")) + DIV + \
    sec(4, '<div class="gh">Next 24 hours</div>' + f'<div class="chartw">{chart()}</div>') + sec(5, hstrip()) + DIV + \
    sec(6, opt_gh("Air") + '<div class="pad">' +
        meter("European AQI", "46 · Fair", 23, "<span>Good</span><span>Fair</span><span>Moderate</span><span>Poor</span><span>Very poor</span>", "aqi") +
        '<div class="tiles in2">' + tile("aqi", "PM2.5", "19µg/m³") + tile("aqi", "NO₂", "36µg/m³") + '</div>' +
        meter("UV index, max today", "0.4 · Low", 4, "<span>0</span><span>3</span><span>6</span><span>8</span><span>11+</span>", "uvm") + '</div>') + \
    sec(7, opt_gh("Pollen") + '<div class="pad pls">' +
        pollen("Grass", "None", 0) + pollen("Birch", "None", 0) + pollen("Alder", "Low", 1) + pollen("Ragweed", "None", 0) + '</div>') + DIV + \
    sec(8, '<div class="gh">Next 7 days</div>' +
        day("Today", "w-rain", "4°", "15°", 30, 74, "2.5mm") +
        day("Wed", "w-partly", "9°", "15°", 57, 74, "0.4mm", True) +
        '<div class="hours">' + "".join([
            hour("00:00", "w-cloud", "9.6°", "", "14km/h"),
            hour("03:00", "w-cloud", "9.2°", "", "16km/h"),
            hour("06:00", "w-rain", "9.0°", "0.2mm", "19km/h"),
            hour("09:00", "w-rain", "10.8°", "0.2mm", "22km/h"),
            hour("12:00", "w-partly", "14.1°", "", "24km/h"),
            hour("15:00", "w-partly", "14.7°", "", "21km/h"),
            hour("18:00", "w-cloud", "12.0°", "", "15km/h"),
            hour("21:00", "w-cloud", "10.4°", "", "12km/h"),
        ]) + '</div>' +
        day("Thu", "w-cloud", "3°", "8°", 26, 46) +
        day("Fri", "w-snow", "-1°", "5°", 9, 34, "1.2mm") +
        day("Sat", "w-cloud", "-2°", "3°", 4, 26) +
        day("Sun", "w-sun", "0°", "6°", 14, 37) +
        day("Mon", "w-partly", "2°", "9°", 20, 51)) + sec(9, ATTR)

B_NOTES = [
    ("Header", "Name of the active location and the age of the last successful fetch. Refresh fetches now; the gear opens Settings (S1).",
     "saved location name · local fetch time"),
    ("Current conditions", "Weather icon, temperature now, a short description and the feels-like temperature. Today's sunrise and sunset on the right.",
     "current.temperature_2m, apparent_temperature, weather_code (icon + text), is_day (day/night icon) · daily.sunrise, daily.sunset"),
    ("Rain alert", "Only shown when rain is expected within the next 2 hours: the first 15-minute step with rain, rounded to the quarter hour. Hidden when it stays dry.",
     "first minutely_15.precipitation &gt; 0 in the next 8 steps"),
    ("24-hour chart", "Line and shaded area: hourly temperature. Grey bars: hourly rain amount. Dots every 3 hours, dashed down to the table; the filled dot is now. Darker background is night. Hover shows any hour.",
     "hourly.temperature_2m, hourly.precipitation · sunrise/sunset for the night shading"),
    ("3-hour table", "One column per dot, directly beneath it: temperature, weather icon, rain chance (only from 20 %), and the time, which doubles as the chart's x-axis.",
     "hourly.temperature_2m, weather_code, precipitation_probability at every third hour"),
    ("Air (optional)", "Off by default; switched on under Settings → Popup → Air quality and UV. European AQI on its banded scale, PM2.5 and NO₂, and today's highest UV. Turning it on adds a second request per refresh.",
     "air-quality-api: current european_aqi, pm2_5, nitrogen_dioxide · daily uv_index_max from the forecast"),
    ("Pollen (optional)", "Off by default; Settings → Popup → Pollen. Four common Austrian pollen types, bucketed from grains/m³ into None / Low / Moderate / High. Europe only, so the toggle is hidden elsewhere.",
     "air-quality-api: current alder_pollen, birch_pollen, grass_pollen, ragweed_pollen"),
    ("Next 7 days", "Same as A: day, icon, rain amount, low, range bar and high on one shared scale. Tap a day to open its hours in 3-hour steps; one day is open at a time. Today starts closed because the chart already covers it. Shown here with Wed open.",
     "daily.weather_code, precipitation_sum, temperature_2m_min / _max · hourly values for the opened day"),
    ("Attribution", "Required by the CC BY 4.0 licence of Open-Meteo's data.", ""),
]

# ============================ SHARED: settings, location, states ============================
def radio(on):
    return f'<span class="rd{" on" if on else ""}"></span>'


settings = header("Settings") + DIV + '<div class="gh">Locations</div>' + "".join([
    row("pin", "Vienna", "Austria · 48.21, 16.37 · 171 m", radio(True) + ib("edit-delete")),
    row("pin", "Innsbruck", "Tyrol, Austria · 574 m", radio(False) + ib("edit-delete")),
]) + f'<div class="row ft"><div class="t"><div class="n">Add location</div></div>{i("list-add")}</div>' + DIV + \
    '<div class="gh">Units</div><div class="pad">' + seg("°C · km/h · mm", "°F · mph · inch") + '</div>' + \
    '<div class="gh">Panel</div>' + "".join([
        row("w-cloud", "Show temperature", None, toggle(True)),
        row("w-rain", "Show rain in the next 2 hours", None, toggle(False)),
        row("thermo", "Show today's high / low", None, toggle(False)),
    ]) + '<div class="gh">Popup</div>' + "".join([
        row("aqi", "Air quality and UV", "Off by default · adds a second request", toggle(True)),
        row("leaf", "Pollen", "Off by default · Europe only", toggle(True)),
    ]) + DIV + row("view-refresh", "Refresh every", None, '<span class="chip">15 min ' + i("pan-down") + '</span>') + \
    row("emblem-system", "Weather model", "Best match picks the best model per location", '<span class="chip">Best match ' + i("pan-down") + '</span>')

search = header("Add location") + '<div class="pad">' + \
    '<div class="in search">' + i("edit-find", "dim") + '<span>Brigittenau</span></div></div>' + DIV + "".join([
        row("pin", "Brigittenau", "Vienna, Austria · 163 m", i("go-next", "dim")),
        row("pin", "Brigittenauer Lände", "Vienna, Austria · 160 m", i("go-next", "dim")),
    ]) + '<div class="pad"><div class="c hint">Search by place or district name. Postcodes only find the city centre, '\
    'so “1200” gives Vienna itself.</div></div>' + DIV + \
    '<div class="pad form">' + '<div class="c">Or enter coordinates</div>' + \
    '<div class="two"><div class="in">48.2428</div><div class="in">16.3755</div></div>' + \
    '<div class="acts"><span class="btn">Cancel</span><span class="btn acc">Add</span></div></div>'

first = header("Weather", back=False, actions=("emblem-system",)) + DIV + \
    '<div class="blank">' + i("pin", "big dim") + '<div class="n">No location yet</div>' + \
    '<div class="c">Choose a place to see its weather. Nothing is sent anywhere until you do.</div>' + \
    '<span class="btn acc">Choose location</span></div>'

stale = header(LOC, '<span class="warnt">' + i("dialog-warning") + 'Updated 2 hours ago</span>', back=False, actions=("view-refresh", "emblem-system")) + DIV + \
    '<div class="banner warn">' + i("dialog-warning") + '<span>Can\'t reach Open-Meteo. Retrying in 4 min. The values below are from 16:02.</span></div>' + \
    '<div class="stale">' + hero() + '</div>' + DIV + \
    '<div class="stale">' + "".join(compact_day(*d) for d in DAYS7[:4]) + '</div>'

limited = header(LOC, UPD, back=False, actions=("view-refresh", "emblem-system")) + DIV + \
    '<div class="banner bad">' + i("dialog-error") + '<span>Open-Meteo refused the request (rate limit reached). '\
    'Next try in 1 hour. Lower the refresh rate in Settings if this keeps happening.</span></div>' + \
    '<div class="pad"><div class="acts"><span class="btn">Open settings</span></div></div>' + DIV + \
    '<div class="gh">Loading</div>' + \
    f'<div class="hero">{i("w-cloud", "w48 dim")}<div><div class="big mut">–</div><div class="c">Fetching the forecast…</div></div>'\
    f'<div class="sun"><span class="spin"></span></div></div>'

SCREENS = "".join([
    PANEL,
    group("Popup: three proposals",
          "<b>A · Classic list</b> is the familiar layout: conditions, six tiles, 7 days that expand into hours. "
          "<b>B · Timeline</b> is about what happens next: a rain alert, a 24-hour curve with a 3-hour table, optional air and pollen, and the same expandable days as A. "
          "<b>C · At a glance plus air</b> keeps the top short and adds air quality, UV and pollen."),
    screen("a1", "A. Classic list", "Plain libcosmic widgets only. Today opens by default with the hours that are left; "
           "tap a day to swap which one is open. One request per refresh.", a_main,
           "current temperature_2m, apparent_temperature, relative_humidity_2m, wind_speed/direction/gusts_10m, pressure_msl, weather_code, is_day · hourly temperature, precipitation, weather_code, wind, visibility · daily min/max, precipitation_sum, sunrise/sunset, uv_index_max."),
    screen("c1", "C. At a glance plus air", "Two requests per refresh: forecast plus the air-quality API. AQI uses the European bands; "
           "pollen is bucketed grains/m³ and the section can be hidden.", c_main,
           "air-quality-api current european_aqi, pm2_5, nitrogen_dioxide, uv_index, alder/birch/grass/ragweed_pollen · forecast API for the rest."),
    '<section class="spacer"></section>',
    annotated("b1", "B. Timeline", "About what happens next. One forecast request per refresh (current, minutely_15, hourly, daily), plus one air-quality request when Air or Pollen is on. "
              "Needs one custom canvas widget for the chart; everything else is plain libcosmic.", b_main, B_NOTES),
    group("Next 24 hours: row order",
          "The chart shows two things: the temperature curve in its upper part and the rain bars at the bottom. "
          "The options differ in whether each table row sits next to the part of the chart it explains."),
    *[order_screen(*o) for o in ORDER_OPTIONS],
    group("Shared screens", "Day detail (used by B, optional for A), settings, adding a location, and the states. The same whichever popup is chosen."),
    screen("b2", "D. Day page", "Opened from a day row. Daily totals as tiles, then 3-hour steps. ← goes back.", b_day,
           "daily wind_speed_10m_max, wind_gusts_10m_max, sunshine_duration, precipitation_hours, precipitation_probability_max, uv_index_max · hourly for that date."),
    screen("s1", "S1. Settings", "Several saved locations, one active; the radio picks which one the panel shows. "
           "Panel extras and popup sections are toggles. Refresh defaults to 15 min; the models update hourly.", settings),
    screen("s2", "S2. Add location", "Search uses Open-Meteo geocoding, no key. Coordinates are the fallback for "
           "anything search can't find.", search, "geocoding-api.open-meteo.com/v1/search?name=…&language=de"),
    screen("s3", "S3. First run", "No location is guessed from the IP. Nothing is fetched until a place is chosen.", first),
    screen("s4", "S4. Offline / stale", "Values stay visible with their age, but dimmed. "
           "After 1 hour the panel button dims too.", stale),
    screen("s5", "S5. Errors and loading", "HTTP 429 backs off for an hour instead of retrying on every refresh. "
           "The first fetch shows a placeholder, never zeros.", limited),
])

CSS = KIT_CSS + WEATHER_CSS + """
.bars4{grid-template-columns:repeat(4,264px)!important;gap:24px!important}
.bars4 .bar{width:264px}
.ap .sub{display:inline-flex;align-items:center;gap:4px;font-weight:400;font-size:12px;color:var(--mut)}
.ap .sub .i{width:12px;height:12px;-webkit-mask-size:12px;mask-size:12px}
.ap.vert{flex-direction:column;gap:0;font-size:11px;line-height:12px;padding:2px 6px}
.grp{margin-top:10px;border-top:1px solid var(--div);padding-top:22px}.grp h2{font-size:17px;line-height:24px;margin:0}
.grp p{margin:4px 0 0;color:var(--mut);max-width:900px}
.spacer{visibility:hidden}
section>p.data{font-size:11px;line-height:15px;margin-top:6px;opacity:.85}
.attr{font-size:11px;color:var(--mut);text-align:center;padding:10px 0 2px}
.hero .c{margin-top:1px}
.lo2{font-size:20px;color:var(--mut);font-weight:400}
.soon{display:flex;align-items:center;gap:8px;margin:0 16px 6px;font-size:13px;color:var(--acc);font-weight:600}
.chartw{padding:0 16px}
.chart{display:block}
.chart text{font-size:10px;fill:var(--mut)}
.chart .tv{fill:var(--on);font-weight:600;font-size:11px}
.chart .line{fill:none;stroke:var(--acc);stroke-width:2;stroke-linejoin:round}
.chart .area{fill:var(--acc);opacity:.12}
.chart .pr{fill:var(--on);opacity:.45}
.chart .base{stroke:var(--div)}
.chart .night{fill:var(--on);opacity:.05}
.chart .dotc{fill:var(--bg);stroke:var(--acc);stroke-width:2}.chart .now{fill:var(--acc)}
.hs{display:grid;padding:2px 16px 8px;text-align:center}
.chart .guide{stroke:var(--div);stroke-dasharray:2 2}
.hs div{display:flex;flex-direction:column;align-items:center;gap:3px;font-size:13px;font-variant-numeric:tabular-nums}
.pp{font-size:11px;color:var(--acc);font-variant-numeric:tabular-nums;min-height:15px;text-align:right}
.hs .pp{text-align:center;line-height:15px}
.b24{padding:0 0 8px;display:flex;flex-direction:column;gap:4px}
.r24{display:grid;padding:0 16px;text-align:center;font-size:13px;font-variant-numeric:tabular-nums}
.r24 div{display:flex;justify-content:center;align-items:center;min-height:16px}
.r24-rain .pp{text-align:center;line-height:15px;min-height:15px}
.cday{display:grid;grid-template-columns:44px 16px 36px 26px 1fr 26px;align-items:center;gap:8px;min-height:32px;padding:0 16px}
.hours.tight{background:transparent}
.glance{display:flex;align-items:center;gap:12px;padding:6px 20px 10px}.glance .gl2{margin-left:4px}
.chips{display:flex;flex-wrap:wrap;gap:6px;padding:0 16px 8px}
.chip2{display:inline-flex;align-items:center;gap:6px;font-size:12px;padding:3px 10px;border-radius:12px;background:var(--comp)}
.chip2 .i{width:13px;height:13px;-webkit-mask-size:13px;mask-size:13px;opacity:.75}
.mt{display:flex;flex-direction:column;gap:4px}
.mh{display:flex;justify-content:space-between;font-size:13px}
.mb{position:relative;height:6px;border-radius:3px}
.mb.aqi{background:linear-gradient(90deg,#50c878 0 20%,#c8d64a 20% 40%,#f2c94c 40% 60%,#f2994a 60% 80%,#eb5757 80%)}
.mb.uvm{background:linear-gradient(90deg,#50c878 0 27%,#f2c94c 27% 55%,#f2994a 55% 73%,#eb5757 73% 91%,#9b51e0 91%)}
.mb i{position:absolute;top:-4px;width:4px;height:14px;margin-left:-2px;border-radius:2px;background:var(--on);box-shadow:0 0 0 2px var(--bg)}
.ms{display:flex;justify-content:space-between;font-size:10px;color:var(--mut)}
.tiles.in2{padding:0}
.pls{gap:6px}
.pl{display:grid;grid-template-columns:1fr 70px 48px;align-items:center;font-size:13px}.pl .c{text-align:right}
.pips{display:flex;gap:3px}.pips i{width:14px;height:6px;border-radius:2px;background:var(--btn)}.pips i.on{background:var(--warn)}
.cols{display:grid;grid-template-columns:repeat(5,1fr);padding:4px 12px 4px;text-align:center}
.cols div{display:flex;flex-direction:column;align-items:center;gap:4px;font-size:13px}
.rd{width:18px;height:18px;border-radius:50%;box-shadow:inset 0 0 0 1.5px var(--mut);display:inline-block}
.rd.on{box-shadow:inset 0 0 0 5px var(--acc)}
.two{display:grid;grid-template-columns:1fr 1fr;gap:8px}
.warnt{display:inline-flex;align-items:center;gap:5px;color:var(--warn)}
.warnt .i{width:12px;height:12px;-webkit-mask-size:12px;mask-size:12px}
.big.mut{color:var(--mut)}
.ghx{display:flex;justify-content:space-between;align-items:center}
.tag{font-size:10px;font-weight:600;color:var(--acc);border:1px solid var(--acc);border-radius:8px;padding:0 7px;line-height:15px;opacity:.85}
.anno{grid-column:1/-1}
.anno .intro{margin:0 0 12px;font-size:12px;line-height:17px;color:var(--mut);max-width:760px}
.annob{display:grid;grid-template-columns:360px 1fr;gap:56px;align-items:start}
.sec{position:relative}
.sec::after{content:attr(data-n);position:absolute;right:-30px;top:50%;transform:translateY(-50%);width:20px;height:20px;border-radius:50%;
  background:var(--acc);color:var(--onacc);font-size:11px;font-weight:700;display:flex;align-items:center;justify-content:center}
.annob{position:relative}
svg.lead{position:absolute;left:0;top:0;pointer-events:none;overflow:visible}
svg.lead path{fill:none;stroke:var(--acc);stroke-width:1;opacity:.55}
.notes{list-style:none;margin:0;padding:0;position:relative;max-width:700px}
.note{display:flex;gap:10px;font-size:13px;line-height:18px;margin-bottom:12px;padding-left:4px}
.note .num{flex:none;width:20px;height:20px;border-radius:50%;background:var(--acc);color:var(--onacc);font-size:11px;font-weight:700;display:flex;align-items:center;justify-content:center}
.note b{display:block;margin-bottom:1px}
.note .src{font-size:11px;line-height:15px;color:var(--mut);margin-top:3px;font-family:"Noto Sans Mono",monospace}
.js .notes{height:var(--h)}.js .note{position:absolute;left:0;right:0;margin:0}
.stale .hero,.stale .cday{opacity:.5}
"""

HTML = f"""<!doctype html>
<html lang="en" data-theme="dark"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Weather applet mockups</title>
<script>document.documentElement.dataset.theme=new URLSearchParams(location.search).get('theme')||'dark'</script>
<style>{CSS}</style></head><body>
<h1>Weather applet (Open-Meteo): design proposals</h1>
<div class="lead">Three popup directions (A, B, C) plus the shared panel button, settings and states. Built with the shared mockup kit; invented data shaped like Open-Meteo responses. Add ?theme=light for the light theme.</div>
<main>{SCREENS}</main>
<script>
// Place each note level with its marker. Overlapping notes form a group that is centred on its markers,
// then a leader line joins every marker to its note.
function place(){{document.querySelectorAll('.anno').forEach(a=>{{
  const box=a.querySelector('.annob'),list=a.querySelector('.notes'),B=box.getBoundingClientRect(),L=list.getBoundingClientRect();
  const notes=[...a.querySelectorAll('.note')].map(n=>{{
    const s=a.querySelector('.sec[data-n="'+n.dataset.n+'"]').getBoundingClientRect();
    const c=s.top+s.height/2-L.top;return {{n,c,h:n.offsetHeight,want:c-10}};}});
  const gap=10;let groups=notes.map(o=>({{items:[o],top:o.want}}));
  const size=g=>g.items.reduce((t,o)=>t+o.h+gap,0)-gap;
  let merged=true;while(merged){{merged=false;
    for(let i=0;i+1<groups.length;i++){{const g=groups[i],h=groups[i+1];
      if(g.top+size(g)+gap>h.top){{const items=g.items.concat(h.items);
        const mean=items.reduce((t,o)=>t+o.want,0)/items.length;const ng={{items,top:0}};
        let off=0,acc=0;items.forEach(o=>{{acc+=off;off=o.h+gap;}});
        ng.top=mean-(size(ng)-items[items.length-1].h)/2;groups.splice(i,2,ng);merged=true;break;}}}}
    if(groups.length&&groups[0].top<0){{groups[0].top=0;merged=merged||groups.length>1;}}}}
  let end=0;groups.forEach(g=>{{let y=Math.max(g.top,end);g.items.forEach(o=>{{o.y=y;o.n.style.top=y+'px';y+=o.h+gap;}});end=y;}});
  list.style.setProperty('--h',end+'px');
  let svg=box.querySelector('svg.lead');if(!svg){{svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.setAttribute('class','lead');box.appendChild(svg);}}
  svg.setAttribute('width',B.width);svg.setAttribute('height',Math.max(B.height,L.top-B.top+end));
  const x0=L.left-B.left-46,x1=L.left-B.left-2,mid=x0+18;
  svg.innerHTML=notes.map(o=>{{const ya=o.c+L.top-B.top,yb=o.y+10+L.top-B.top;
    return `<path d="M${{x0}} ${{ya}} H${{mid}} L${{mid+14}} ${{yb}} H${{x1}}"/>`;}}).join('');}});}}
document.documentElement.classList.add('js');addEventListener('load',place);addEventListener('resize',place);
</script>
</body></html>
"""
open(OUT, "w").write(HTML)
print(len(HTML))

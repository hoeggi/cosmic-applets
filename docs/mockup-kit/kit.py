"""Shared building blocks for the applet mockups in docs/*-mockups: COSMIC theme icons, placeholder
glyphs, popup widgets as HTML snippets, and the CSS that styles them in COSMIC's dark and light colours.

A mockup script imports this module, writes its screens with these helpers, and appends its own CSS:

    sys.path.insert(0, os.path.join(HERE, "..", "mockup-kit"))
    from kit import *

Icons are read from the installed icon themes at build time, so building needs a COSMIC desktop.
"""
import base64, glob

__all__ = [
    "uri", "theme_icon", "glyph", "GLYPHS", "i", "ib", "toggle", "press", "row", "val", "dot", "header",
    "DIV", "field", "footer", "cb", "pick", "seg", "screen", "bar", "tile", "rng", "day", "hour", "CSS",
    "WEATHER_CSS",
]

# COSMIC lacks some icons (battery, power profiles); the desktop falls back to Pop, then Adwaita.
THEMES = ["/usr/share/icons/Cosmic/scalable/*", "/usr/share/icons/Pop/scalable/*",
          "/usr/share/icons/Adwaita/symbolic/*"]


def uri(svg):
    return "data:image/svg+xml;base64," + base64.b64encode(svg.encode()).decode()


def theme_icon(name):
    for theme in THEMES:
        hits = glob.glob(f"{theme}/{name}-symbolic.svg")
        if hits:
            return uri(open(hits[0]).read())
    raise SystemExit(f"icon not found: {name}")


# Placeholder glyphs for things no icon theme has.
def glyph(body):
    return uri(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><g fill="#000">{body}</g></svg>')


GLYPHS = {
    "thermo": glyph('<path d="M6 3a2 2 0 1 1 4 0v6.2a3.5 3.5 0 1 1-4 0zm1.5 0v7l-.4.2a2 2 0 1 0 1.8 0l-.4-.2V3a.5.5 0 0 0-1 0z"/><circle cx="8" cy="12" r="1.3"/><rect x="7.6" y="6" width=".8" height="5"/>'),
    "bolt": glyph('<path d="M9.5 1 3 9h4l-1 6 7-8.5H8.8z"/>'),
    "bulb": glyph('<path d="M8 1a5 5 0 0 0-3 9c.6.5 1 1 1 1.7V12h4v-.3c0-.7.4-1.2 1-1.7a5 5 0 0 0-3-9zM6 13h4v1H6zm1 1.5h2V15H7z"/>'),
    "drop": glyph('<path d="M8 1S3 6.5 3 10a5 5 0 0 0 10 0C13 6.5 8 1 8 1z"/>'),
    "scene": glyph('<path d="M8 1l1.8 4.4L14.5 6 11 9l1 4.6L8 11.2 4 13.6 5 9 1.5 6l4.7-.6z"/>'),
    "door": glyph('<path d="M4 1h8v14H4zm6 6.5a.8.8 0 1 0 0 1.6.8.8 0 0 0 0-1.6z" fill-rule="evenodd"/>'),
    "drag": glyph('<circle cx="6" cy="4" r="1.2"/><circle cx="10" cy="4" r="1.2"/><circle cx="6" cy="8" r="1.2"/><circle cx="10" cy="8" r="1.2"/><circle cx="6" cy="12" r="1.2"/><circle cx="10" cy="12" r="1.2"/>'),
}
for name in [
    "emblem-system", "go-previous", "go-next", "list-add", "pencil", "changes-prevent", "edit-find",
    "go-up", "go-down", "user-home", "dialog-warning", "dialog-error", "object-select", "edit-delete",
    "view-refresh", "pan-down", "image-red-eye", "media-playback-start",
]:
    GLYPHS[name] = theme_icon(name)

# Weather glyphs (placeholders; the real applets use the theme's weather icons).
_CLOUD = '<path d="M4.5 13a3 3 0 0 1-.4-6A4 4 0 0 1 11.7 6 3.5 3.5 0 0 1 11.5 13z"/>'
_SMALL = '<path d="M4.5 10a2.5 2.5 0 0 1-.3-5A3.5 3.5 0 0 1 11 4.3 3 3 0 0 1 11 10z"/>'
_SUN = '<circle cx="8" cy="8" r="3"/><path d="M7.4 1h1.2v2.4H7.4zM7.4 12.6h1.2V15H7.4zM1 7.4h2.4v1.2H1zM12.6 7.4H15v1.2h-2.4zM2.7 3.6l.9-.9 1.7 1.7-.9.9zM10.7 11.6l.9-.9 1.7 1.7-.9.9zM2.7 12.4l1.7-1.7.9.9-1.7 1.7zM10.7 4.4l1.7-1.7.9.9-1.7 1.7z"/>'
GLYPHS.update({
    "w-sun": glyph(_SUN),
    "w-partly": glyph('<circle cx="5.5" cy="5.5" r="2.4"/><path d="M5 .6h1v1.6H5zM.6 5h1.6v1H.6zM1.6 2.3l.7-.7 1.1 1.1-.7.7zM8.3 1.6l.7.7-1.1 1.1-.7-.7z"/><path d="M6.5 14a2.6 2.6 0 0 1-.3-5.2A3.5 3.5 0 0 1 12.8 8 3 3 0 0 1 12.5 14z"/>'),
    "w-cloud": glyph(_CLOUD),
    "w-rain": glyph(_SMALL + '<path d="M4.6 11.2h1.1l-1 3.3H3.6zM7.6 11.2h1.1l-1 3.3H6.6zM10.6 11.2h1.1l-1 3.3H9.6z"/>'),
    "w-snow": glyph(_SMALL + '<circle cx="4.5" cy="12.5" r="1"/><circle cx="8" cy="14" r="1"/><circle cx="11.5" cy="12.5" r="1"/>'),
    "w-storm": glyph(_SMALL + '<path d="M8.8 10.5 6 13h1.8l-1 2.5L10 12.4H8.2z"/>'),
    "w-rise": glyph('<path d="M2 12h12v1.2H2zM4.8 11a3.2 3.2 0 0 1 6.4 0zM7.4 7V4.2H6L8 1.8l2 2.4H8.6V7z"/>'),
    "w-set": glyph('<path d="M2 12h12v1.2H2zM4.8 11a3.2 3.2 0 0 1 6.4 0zM7.4 1.8V4.600H6L8 7l2-2.400H8.600V1.800z"/>'),
    "w-fog": glyph('<path d="M2 5h12v1.3H2zM3.5 8H13v1.3H3.5zM2 11h10v1.3H2z"/>'),
    "wind": glyph('<path d="M1 7h8.5a1.7 1.7 0 1 0-1.7-1.7H6.5A3 3 0 1 1 9.5 8.3H1zM1 10h10.5a1.4 1.4 0 1 1-1.4 1.4H8.8a2.7 2.7 0 1 0 2.7-2.7H1z"/>'),
    "gauge": glyph('<path d="M8 2a7 7 0 0 0-5.6 11.2l1-.8A5.7 5.7 0 1 1 12.6 12.4l1 .8A7 7 0 0 0 8 2zm3.5 3.6L7.3 8.2a1.2 1.2 0 1 0 1.5 1.6z"/>'),
    "dew": glyph('<path d="M5 2S2 5.5 2 8a3 3 0 0 0 6 0C8 5.5 5 2 5 2zM11 7s-2.5 3-2.5 5a2.5 2.5 0 0 0 5 0c0-2-2.5-5-2.5-5z"/>'),
    "solar": glyph(_SUN),
})


# ---------- widgets ----------
def i(name, cls=""):
    return f'<span class="i {cls}" style="--i:url({GLYPHS[name]})"></span>'


def ib(name):
    return f'<span class="ib">{i(name)}</span>'


def toggle(on, disabled=False):
    return f'<span class="tg{" on" if on else ""}{" dis" if disabled else ""}"><b></b></span>'


def press(label="Press", disabled=False):
    return f'<span class="btn{" dis" if disabled else ""}">{label}</span>'


def row(icon, name, sub=None, right="", cls=""):
    text = f'<div class="t"><div class="n">{name}</div>' + (f'<div class="c">{sub}</div>' if sub else "") + "</div>"
    return f'<div class="row {cls}">{i(icon)}{text}<div class="r">{right}</div></div>'


def val(v):
    return f'<span class="v">{v}</span>'


def dot(kind, label):
    return f'<span class="st"><span class="dot {kind}"></span>{label}</span>'


def header(title, status=None, back=True, actions=()):
    b = ib("go-previous") if back else ""
    s = f'<div class="c">{status}</div>' if status else ""
    a = "".join(ib(x) for x in actions)
    return f'<div class="hd">{b}<div class="t"><div class="h">{title}</div>{s}</div>{a}</div>'


DIV = '<div class="dv"></div>'


def field(label, value, extra="", cls=""):
    return f'<div class="f {cls}"><div class="c">{label}</div><div class="in">{value}{extra}</div></div>'


def footer(label, icon="go-next"):
    return f'{DIV}<div class="row ft"><div class="t"><div class="n">{label}</div></div>{i(icon)}</div>'


def cb(on, disabled=False):
    inner = i("object-select") if on else ""
    return f'<span class="cb{" on" if on else ""}{" dis" if disabled else ""}">{inner}</span>'


def pick(on, name, right, disabled=False):
    return f'<div class="row pick{" muted" if disabled else ""}">{cb(on, disabled)}<div class="t"><div class="n">{name}</div></div><div class="r"><span class="c">{right}</span></div></div>'


def seg(a, b, active=0):
    return f'<div class="seg"><span class="{"on" if active == 0 else ""}">{a}</span><span class="{"on" if active == 1 else ""}">{b}</span></div>'


def screen(key, title, note, body):
    return f'<section id="{key}"><h2>{title}</h2><div class="popup">{body}</div><p>{note}</p></section>'


def bar(icon, kind=""):
    return f'<div class="bar"><span class="clock">Sat Oct 31 21:14</span><span class="sp"></span>{icon}<span class="tray"><i></i><i></i><i></i></span></div>'


# ---------- weather widgets (style them with WEATHER_CSS) ----------
def tile(icon, label, value):
    return f'<div class="tile">{i(icon, "dim")}<div><div class="c">{label}</div><div class="v">{value}</div></div></div>'


def rng(lo, hi, low=0, high=100):
    return f'<span class="rng"><b style="left:{lo}%;right:{100 - hi}%"></b></span>'


def day(name, icon, low, high, a, b, rain="", open_=False):
    chev = "pan-down" if open_ else "go-next"
    r = f'<span class="rain">{rain}</span>' if rain else '<span class="rain"></span>'
    return (f'<div class="day{" open" if open_ else ""}"><span class="dn">{name}</span>{i(icon)}{r}'
            f'<span class="lo">{low}</span>{rng(a, b)}<span class="v hi">{high}</span>{i(chev, "dim")}</div>')


def hour(time, icon, temp, rain, wind):
    return (f'<div class="hr"><span>{time}</span>{i(icon)}<span class="v">{temp}</span>'
            f'<span class="c">{rain}</span><span class="c e">{wind}</span></div>')


# ---------- CSS ----------
CSS = """
:root{--bg:#1b1b1b;--on:#c0c0c0;--comp:#2e2e2e;--hover:#434343;--pri:#272727;--div:rgba(192,192,192,.2);--acc:#63d0df;--onacc:#000;--ok:#92cf9c;--bad:#fda1a0;--warn:#f7e062;--btn:rgba(158,158,158,.25);--page:#101010;--desk:#0d3b42;--panel:#1b1b1b;--mut:rgba(192,192,192,.65)}
html[data-theme=light]{--bg:#d7d7d7;--on:#272727;--comp:#f5f5f5;--hover:#f6f6f6;--pri:#ebebeb;--div:rgba(39,39,39,.2);--acc:#00525a;--onacc:#fff;--ok:#185529;--bad:#78292e;--warn:#534800;--btn:rgba(46,46,46,.25);--page:#f2f2f2;--desk:#8fc7cd;--panel:#d7d7d7;--mut:rgba(39,39,39,.68)}
*{box-sizing:border-box}
body{margin:0;background:var(--page);color:var(--on);font:14px/20px "Open Sans","Noto Sans",sans-serif;padding:32px}
main{display:grid;grid-template-columns:repeat(4,360px);gap:36px 32px;width:max-content;align-items:start}
h1{font-size:22px;line-height:30px;margin:0;font-weight:600}
.lead{margin:2px 0 26px;color:var(--mut)}
h2{font-size:13px;line-height:18px;font-weight:600;margin:0 0 10px}
section>p{font-size:12px;line-height:17px;color:var(--mut);margin:10px 2px 0}
.wide{grid-column:1/-1}
.bars{display:grid;grid-template-columns:repeat(4,360px);gap:32px}
figure{margin:0}figcaption{font-size:12px;color:var(--mut);margin-top:8px}
.bar{height:36px;background:var(--panel);border-radius:8px;display:flex;align-items:center;padding:0 10px;gap:6px;font-size:13px}
.clock{font-size:12px}.sp{flex:1}
.tray{display:flex;gap:9px;margin-left:8px}.tray i{width:14px;height:14px;border-radius:3px;background:var(--on);opacity:.35}
.ap{display:inline-flex;align-items:center;gap:6px;padding:4px 8px;border-radius:14px;position:relative;font-weight:600}
.ap.on{background:var(--hover)}.ap.dimmed{opacity:.4}
.badge{position:absolute;right:4px;top:3px;width:8px;height:8px;border-radius:50%;background:var(--bad);border:1.5px solid var(--panel)}
.popup{width:360px;background:var(--bg);border-radius:16px;padding:8px 0;box-shadow:0 4px 16px rgba(0,0,0,.35)}
.i{display:inline-block;width:16px;height:16px;flex:none;background:currentColor;-webkit-mask:var(--i) center/16px no-repeat;mask:var(--i) center/16px no-repeat}
.i.dim{opacity:.6}.i.big{width:32px;height:32px;-webkit-mask-size:32px;mask-size:32px}
.hd{display:flex;align-items:center;gap:8px;padding:4px 12px 8px}
.hd .t{flex:1;min-width:0}.h{font-weight:600}
.c{font-size:12px;line-height:16px;color:var(--mut)}
.dv{height:1px;background:var(--div);margin:4px 12px}
.row{display:flex;align-items:center;gap:12px;min-height:44px;padding:5px 16px}
.row .t{flex:1;min-width:0}.row .n{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.r{display:flex;align-items:center;gap:8px;flex:none}
.v{font-weight:600;font-variant-numeric:tabular-nums}
.ro .n{opacity:.8}.ro>.i:first-child{opacity:.7}
.ft{min-height:40px}
.ib{width:28px;height:28px;border-radius:14px;display:inline-flex;align-items:center;justify-content:center;background:transparent}
.ib.dis{opacity:.3}
.tg{width:44px;height:24px;border-radius:12px;background:var(--btn);display:inline-block;position:relative;flex:none}
.tg b{position:absolute;top:3px;left:3px;width:18px;height:18px;border-radius:50%;background:var(--on)}
.tg.on{background:var(--acc)}.tg.on b{left:23px;background:var(--onacc)}
.tg.dis{opacity:.4}
.btn{display:inline-flex;align-items:center;height:32px;padding:0 16px;border-radius:16px;background:var(--btn);font-weight:600;font-size:13px;flex:none}
.btn.acc{background:var(--acc);color:var(--onacc)}.btn.dis{opacity:.4}
.st{display:inline-flex;align-items:center;gap:6px}
.dot{width:8px;height:8px;border-radius:50%;background:var(--ok);flex:none}
.dot.off{background:transparent;box-shadow:inset 0 0 0 1.5px var(--mut)}.dot.bad{background:var(--bad)}
.pad{padding:0 12px 8px;display:flex;flex-direction:column;gap:10px}
.seg{display:grid;grid-template-columns:1fr 1fr;background:var(--btn);border-radius:16px;padding:2px;text-align:center;font-size:13px;font-weight:600}
.seg span{padding:5px 0;border-radius:14px}.seg .on{background:var(--acc);color:var(--onacc)}
.in{position:relative;display:flex;align-items:center;gap:8px;height:32px;border-radius:8px;background:var(--comp);padding:0 12px;box-shadow:inset 0 0 0 1px var(--div)}
.ph{color:var(--mut)}.end{position:absolute;right:10px}
.tools{display:flex;align-items:center;justify-content:space-between}
.chip{display:inline-flex;align-items:center;gap:6px;font-size:13px;font-weight:600;padding:4px 12px;border-radius:14px;background:var(--btn)}
.link{color:var(--acc);font-size:13px;font-weight:600}
.gh{font-size:12px;font-weight:600;color:var(--mut);padding:8px 16px 2px}
.pick{min-height:36px;padding:3px 16px}
.pick.muted .n{opacity:.5}
.cb{width:18px;height:18px;border-radius:4px;box-shadow:inset 0 0 0 1.5px var(--mut);display:inline-flex;align-items:center;justify-content:center;flex:none}
.cb.on{background:var(--acc);box-shadow:none;color:var(--onacc)}.cb.dis{opacity:.35}
.hint{padding:0 4px}
.form{gap:12px;padding-top:4px}.f{display:flex;flex-direction:column;gap:4px}
.ok{display:flex;gap:8px;align-items:center;color:var(--ok);font-size:12px;line-height:16px}
.acts{display:flex;gap:8px;align-items:center;justify-content:flex-end;margin-top:2px}
.blank{display:flex;flex-direction:column;align-items:center;gap:4px;padding:30px 24px 26px;text-align:center}
.blank .n{font-weight:600;margin-top:8px}.blank .btn{margin-top:14px}
.banner{display:flex;gap:10px;margin:6px 12px;padding:10px 12px;border-radius:8px;font-size:12px;line-height:16px;background:var(--pri)}
.banner .i{margin-top:1px}.banner.warn .i{color:var(--warn)}.banner.bad .i{color:var(--bad)}
.stale .row .t,.stale .row>.i,.stale .v{opacity:.5}
.err{color:var(--bad)}
.spin{width:16px;height:16px;border-radius:50%;border:2px solid var(--div);border-top-color:var(--acc);display:inline-block}
"""

WEATHER_CSS = """
main{grid-template-columns:repeat(3,360px)}
.bars{grid-template-columns:repeat(3,360px)}
.i.w32{width:32px;height:32px;-webkit-mask-size:32px;mask-size:32px}
.i.w48{width:56px;height:56px;-webkit-mask-size:56px;mask-size:56px}
.wcard{display:flex;align-items:center;gap:12px;padding:8px 16px;min-height:58px}
.wcard .t{flex:1;min-width:0}
.temp{font-size:20px;line-height:26px;font-weight:600;font-variant-numeric:tabular-nums}
.temp.mut{color:var(--mut)}
.hl{font-variant-numeric:tabular-nums}
.wcard.old .i.w32,.wcard.old .temp{opacity:.5}
.warn{display:flex;align-items:center;gap:5px;color:var(--warn)}
.warn .i{width:12px;height:12px;-webkit-mask-size:12px;mask-size:12px}
.hero{display:flex;align-items:center;gap:16px;padding:8px 20px 12px}
.sun{margin-left:auto;display:flex;flex-direction:column;gap:6px;font-size:13px;font-variant-numeric:tabular-nums}.sun span{display:flex;align-items:center;gap:8px}
.sunrow{display:flex;justify-content:space-between;padding:8px 20px;font-size:13px;font-variant-numeric:tabular-nums}.sunrow span{display:flex;align-items:center;gap:8px}
.big{font-size:34px;line-height:40px;font-weight:600;font-variant-numeric:tabular-nums}
.tiles{display:grid;grid-template-columns:1fr 1fr;gap:6px;padding:0 12px 8px}
.tile{display:flex;align-items:center;gap:10px;padding:7px 10px;background:var(--comp);border-radius:8px}
.tile .v{font-size:13px;line-height:18px}
.day{display:grid;grid-template-columns:40px 16px 40px 24px 1fr 24px 16px;align-items:center;gap:7px;min-height:36px;padding:2px 16px}
.day.open{background:var(--comp)}
.dn{font-weight:600;font-size:13px}
.rain{font-size:11px;color:var(--mut);text-align:right;font-variant-numeric:tabular-nums}
.lo{font-size:13px;color:var(--mut);text-align:right;font-variant-numeric:tabular-nums}
.hi{font-size:13px;text-align:right}
.rng{position:relative;height:5px;border-radius:3px;background:var(--btn);display:block}
.rng b{position:absolute;top:0;bottom:0;border-radius:3px;background:var(--acc)}
.hours{background:var(--comp);padding:0 16px 6px}
.hr{display:grid;grid-template-columns:44px 16px 46px 1fr 62px;gap:9px;align-items:center;height:28px;font-size:13px;font-variant-numeric:tabular-nums}
.hr .c{text-align:right}
"""

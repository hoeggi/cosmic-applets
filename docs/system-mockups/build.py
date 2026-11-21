#!/usr/bin/env python3
"""Builds docs/system-mockups/index.html: design for one combined system applet
(audio, battery, Bluetooth, network, power) to replace five separate COSMIC applets.

Built from docs/mockup-kit, so all mockups share one look.
All values are invented. Usage: python3 build.py index.html
"""
import os, sys

OUT = sys.argv[1]
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "mockup-kit"))
from kit import GLYPHS, i, ib, row, val, dot, header, DIV, toggle, seg, cb, theme_icon, CSS as BASE_CSS

for n in [
    "audio-volume-high", "audio-volume-medium", "audio-volume-muted", "microphone-sensitivity-high",
    "microphone-sensitivity-muted", "audio-input-microphone", "audio-speakers", "audio-headphones", "audio-headset",
    "bluetooth-active", "bluetooth-disabled", "network-wireless-signal-excellent",
    "network-wireless-signal-good", "network-wireless-signal-ok", "network-wireless-signal-weak",
    "network-wireless-encrypted", "network-wireless-acquiring", "network-wireless-no-route",
    "network-wired", "preferences-vpn", "airplane-mode", "display-brightness",
    "battery-level-80", "battery-level-70-charging", "battery-level-20", "battery-level-10",
    "battery-level-100-charged", "power-profile-balanced", "power-profile-performance",
    "power-profile-power-saver", "system-shutdown", "system-reboot", "system-suspend",
    "system-lock-screen", "system-log-out", "input-mouse", "input-keyboard", "phone", "computer",
    "pan-up", "emblem-ok",
]:
    GLYPHS[n] = theme_icon(n)


# ---------- extra widgets ----------
def slider(icon_name, pct, right=None, muted=False):
    r = right if right is not None else f'<span class="v pct">{pct}%</span>'
    return (f'<div class="sl{" mute" if muted else ""}">{ib(icon_name)}<div class="track"><b style="width:{pct}%"></b>'
            f'<i style="left:{pct}%"></i></div>{r}</div>')


def tile(icon_name, name, sub, on=True, more=True):
    m = f'<span class="more">{i("go-next")}</span>' if more else ""
    return (f'<div class="tile{" on" if on else ""}">{i(icon_name)}<div class="t"><div class="n">{name}</div>'
            f'<div class="c">{sub}</div></div>{m}</div>')


def tiles(*ts):
    return '<div class="tiles">' + "".join(ts) + "</div>"


def power_row(active=None):
    btns = [("system-lock-screen", "Lock"), ("system-log-out", "Log out"), ("system-suspend", "Suspend"),
            ("system-reboot", "Restart"), ("system-shutdown", "Shut down"), ("emblem-system", "Settings")]
    return '<div class="pw">' + "".join(
        f'<span class="pb{" on" if n == active else ""}" title="{n}">{i(ic)}</span>' for ic, n in btns) + "</div>" + \
        '<div class="row ft applet"><div class="t"><div class="n">Applet settings…</div></div></div>'


def battery_head(pct=78, text="3 h 40 min left", icon_name="battery-level-80", profile="Balanced", low=False):
    return (f'<div class="bh{" low" if low else ""}">{i(icon_name, "w24")}<div class="t"><div><span class="big2">{pct}%</span>'
            f'<span class="c"> · {text}</span></div><div class="meter"><b style="width:{pct}%"></b></div></div>'
            f'<span class="c prof">{profile}</span><span class="more">{i("go-next")}</span></div>')


def choice(icon_name, name, sub, on=False, right=""):
    mark = i("object-select") if on else ""
    return (f'<div class="row ch{" on" if on else ""}">{i(icon_name)}<div class="t"><div class="n">{name}</div>'
            + (f'<div class="c">{sub}</div>' if sub else "") + f'</div><div class="r">{right}<span class="mk">{mark}</span></div></div>')


def gh(text, right=""):
    return f'<div class="gh flex"><span>{text}</span>{right}</div>'


def screen(key, title, note, body, tag=None):
    t = f'<span class="tag">{tag}</span>' if tag else ""
    return f'<section id="{key}"><h2>{title}{t}</h2><div class="popup">{body}</div><p>{note}</p></section>'


def heading(text, sub):
    return f'<section class="wide sh"><h3>{text}</h3><p>{sub}</p></section>'


# ---------- main page ----------
main_page = (
    battery_head()
    + DIV
    + slider("audio-volume-high", 64)
    + slider("microphone-sensitivity-high", 80)
    + slider("display-brightness", 55)
    + tiles(
        tile("network-wireless-signal-excellent", "Network", "Hoeggi-5G"),
        tile("bluetooth-active", "Bluetooth", "2 connected"),
        tile("audio-headphones", "Output", "WH-1000XM5"),
        tile("airplane-mode", "Airplane", "Off", on=False, more=False),
    )
    + DIV
    + power_row()
)

# ---------- detail pages, opened by the chevrons ----------
b_audio = header("Sound") + DIV + slider("audio-volume-high", 64) + gh("Output") + "".join([
    choice("audio-headphones", "WH-1000XM5", "Bluetooth", True),
    choice("audio-speakers", "Speakers", "Built-in audio"),
    choice("computer", "DELL U2723QE", "HDMI / DisplayPort"),
]) + DIV + slider("microphone-sensitivity-high", 80) + gh("Input") + "".join([
    choice("audio-input-microphone", "Built-in microphone", "Internal", True),
    choice("audio-headset", "WH-1000XM5", "Bluetooth · lowers playback quality"),
]) + DIV + '<div class="row ft"><div class="t"><div class="n">Sound settings…</div></div></div>'

def sect(title, right=""):
    return f'<div class="sect"><span>{title}</span>{right}</div>'


def pill(text):
    return f'<span class="pill">{text}</span>'


LOCK = i("network-wireless-encrypted", "dim")


def network_page(wired=None, wifi_sub="Connected · 866 Mb/s", wifi_pill="", vpn_on=False):
    """Wired only appears when the hardware has an Ethernet port. The pill marks the connection
    that carries traffic (NetworkManager's default route); the other one stays connected."""
    body = header("Network", actions=("view-refresh",))
    if wired:
        sub, on, pl = wired
        body += DIV + sect("Wired") + choice("network-wired", "Ethernet", sub, on, pl)
    body += DIV + sect("Wi-Fi", toggle(True)) + choice("network-wireless-signal-excellent", "Hoeggi-5G", wifi_sub, True, wifi_pill + LOCK)
    body += '<div class="gh2">Other networks</div>' + "".join([
        choice("network-wireless-signal-good", "Hoeggi-2G", None, right=LOCK),
        choice("network-wireless-signal-ok", "FRITZ!Box 7590 XY", None, right=LOCK),
        choice("network-wireless-signal-weak", "Guest", "Open network"),
    ])
    vpn_sub = "Connected via Ethernet · 1 h 12 min" if vpn_on else "Off"
    body += DIV + sect("VPN") + choice("preferences-vpn", "Office VPN", vpn_sub, vpn_on, toggle(vpn_on)) \
        + choice("preferences-vpn", "Home WireGuard", "Off", right=toggle(False))
    return body + DIV + '<div class="row ft"><div class="t"><div class="n">Network settings…</div></div></div>'


b_network = network_page(wired=("Cable unplugged", False, ""))
b_network_both = network_page(
    wired=("Connected · 1 Gb/s", True, pill("In use")),
    wifi_sub="Connected · standby", vpn_on=True)

b_bt = header("Bluetooth", actions=("view-refresh",)) + '<div class="row"><div class="t"><div class="n">Bluetooth</div></div><div class="r">' + toggle(True) + "</div></div>" + DIV + \
    gh("My devices") + "".join([
        choice("audio-headphones", "WH-1000XM5", "Connected · 90%", True, '<span class="link sm">Disconnect</span>'),
        choice("input-mouse", "MX Master 3S", "Connected · 45%", True, '<span class="link sm">Disconnect</span>'),
        choice("input-keyboard", "MX Keys", "Not connected", right='<span class="link sm">Connect</span>'),
    ]) + gh("Nearby", '<span class="spin"></span>') + "".join([
        choice("phone", "Pixel 9", None, right='<span class="link sm">Pair</span>'),
        choice("bluetooth-active", "JBL Flip 6", None, right='<span class="link sm">Pair</span>'),
    ]) + DIV + '<div class="row ft"><div class="t"><div class="n">Bluetooth settings…</div></div></div>'

b_power = header("Battery") + '<div class="pad">' + \
    '<div class="bh2"><span class="big2">78%</span><span class="c">3 h 40 min left · 11.2 W</span></div>' + \
    '<div class="meter lg"><b style="width:78%"></b></div></div>' + DIV + gh("Power mode") + "".join([
        choice("power-profile-power-saver", "Battery saver", "Lower performance, longer runtime"),
        choice("power-profile-balanced", "Balanced", "Standard performance and power use", True),
        choice("power-profile-performance", "Performance", "Higher performance and power use"),
    ]) + DIV + '<div class="row"><div class="t"><div class="n">Limit charge to 80%</div><div class="c">Protects the battery when mostly plugged in</div></div><div class="r">' + toggle(True) + "</div></div>" + \
    gh("Connected devices") + "".join([
        row("audio-headphones", "WH-1000XM5", None, val("90%")),
        row("input-mouse", "MX Master 3S", None, val("45%")),
    ]) + DIV + '<div class="row ft"><div class="t"><div class="n">Power settings…</div></div></div>'

confirm = header("System", back=False) + DIV + \
    '<div class="blank">' + i("system-shutdown", "big") + '<div class="n">Shut down?</div><div class="c">The computer shuts down automatically in 54 seconds.</div>' + \
    '<div class="acts mid"><span class="btn">Cancel</span><span class="btn danger">Shut down now</span></div></div>' + DIV + power_row("Shut down")

# ---------- states ----------
both = battery_head(100, "Fully charged", "battery-level-100-charged") + DIV + \
    slider("audio-volume-high", 40) + slider("microphone-sensitivity-high", 80) + slider("display-brightness", 70) + tiles(
        tile("network-wired", "Network", "Ethernet · VPN"),
        tile("bluetooth-active", "Bluetooth", "MX Keys"),
        tile("audio-speakers", "Output", "DELL U2723QE"),
        tile("airplane-mode", "Airplane", "Off", on=False, more=False),
    ) + DIV + power_row()

low = battery_head(9, "about 25 min left", "battery-level-10", "Battery saver", low=True) + \
    '<div class="banner bad">' + i("dialog-warning") + '<span>Battery low. Battery saver switched on automatically. Plug in soon.</span></div>' + DIV + \
    slider("audio-volume-high", 64) + slider("display-brightness", 30) + tiles(
        tile("network-wireless-signal-good", "Network", "Hoeggi-5G"),
        tile("bluetooth-disabled", "Bluetooth", "Off", on=False),
        tile("audio-speakers", "Output", "Speakers"),
        tile("airplane-mode", "Airplane", "Off", on=False, more=False),
    ) + DIV + power_row()

airplane = battery_head(100, "Fully charged", "battery-level-100-charged") + DIV + \
    slider("audio-volume-muted", 0, '<span class="c pct">Muted</span>', muted=True) + slider("display-brightness", 55) + tiles(
        tile("network-wireless-no-route", "Network", "Off", on=False),
        tile("bluetooth-disabled", "Bluetooth", "Off", on=False),
        tile("audio-speakers", "Output", "Speakers"),
        tile("airplane-mode", "Airplane", "On", more=False),
    ) + DIV + power_row()

desktop = slider("audio-volume-high", 40) + slider("microphone-sensitivity-muted", 0, '<span class="c pct">Muted</span>', muted=True) + tiles(
    tile("network-wired", "Network", "Ethernet"),
    tile("bluetooth-active", "Bluetooth", "MX Keys"),
    tile("audio-speakers", "Output", "DELL U2723QE"),
    tile("preferences-vpn", "VPN", "Office VPN", on=False),
) + DIV + power_row()

settings = header("Settings") + DIV + gh("Sections, in this order") + "".join(
    f'<div class="row">{i("drag", "dim")}{i(ic)}<div class="t"><div class="n">{n}</div></div><div class="r">{toggle(on)}</div></div>'
    for ic, n, on in [
        ("battery-level-80", "Battery and power mode", True),
        ("audio-volume-high", "Volume", True),
        ("microphone-sensitivity-high", "Microphone", True),
        ("display-brightness", "Brightness", True),
        ("network-wireless-signal-excellent", "Network", True),
        ("bluetooth-active", "Bluetooth", True),
        ("system-shutdown", "Power buttons", True),
    ]) + DIV + gh("Panel icons") + "".join(
    f'<div class="row pick">{cb(on)}{i(ic)}<div class="t"><div class="n">{n}</div>' + (f'<div class="c">{sub}</div>' if sub else "") + '</div></div>'
    for on, ic, n, sub in [
        (True, "network-wireless-signal-excellent", "Network", None),
        (True, "preferences-vpn", "VPN", "Only while a VPN is connected"),
        (False, "bluetooth-active", "Bluetooth", None),
        (False, "audio-volume-high", "Volume", None),
        (False, "microphone-sensitivity-high", "Microphone", "Only while an app is recording"),
        (True, "battery-level-80", "Battery", None),
    ]) + "".join([
        '<div class="row"><div class="t"><div class="n">Battery percentage</div></div><div class="r">' + toggle(True) + "</div></div>",
        '<div class="row"><div class="t"><div class="n">Show alerts</div><div class="c">Unpicked icons appear in red while something needs attention</div></div><div class="r">' + toggle(True) + "</div></div>",
    ]) + DIV + gh("Power") + \
    '<div class="row"><div class="t"><div class="n">Ask before shutting down</div></div><div class="r">' + toggle(True) + "</div></div>"


# ---------- panel ----------
def bar(content):
    return f'<div class="bar"><span class="clock">Sat Nov 21 21:14</span><span class="sp"></span>{content}<span class="tray"><i></i><i></i></span></div>'


def ap(*icons, label="", cls=""):
    lab = f"<span>{label}</span>" if label else ""
    return f'<span class="ap {cls}">' + "".join(i(x) for x in icons) + lab + "</span>"


PANEL = '<section class="wide"><h2>Panel: one button for all five areas</h2><div class="bars">' + "".join(
    f"<figure>{bar(c)}<figcaption>{t}</figcaption></figure>" for c, t in [
        (ap("network-wireless-signal-excellent", "bluetooth-active", "audio-volume-high", "battery-level-80", label="78%"),
         "Default: every icon picked, battery percentage on"),
        (ap("network-wired", "preferences-vpn", "bluetooth-active", "audio-volume-high", "battery-level-100-charged", label="100%"),
         "Ethernet in use, VPN up: the network icon follows the connection in use, the key shows while a VPN is connected"),
        (ap("network-wireless-signal-excellent", "battery-level-80", label="78%"),
         "Picked: network and battery only, as in Settings (12). Bluetooth and volume stay hidden"),
        (ap("network-wireless-no-route", "audio-volume-muted", "battery-level-10", label="9%", cls="alert"),
         "Show alerts on: problems turn red, and unpicked icons (muted volume) appear while the problem lasts. Full list under Settings (12)"),
    ]) + "</div></section>"


SCREENS = "".join([
    heading("Main popup",
            "Everything everyday fits on one page. Every chevron (battery header and tiles) opens that area's detail page. Bluetooth, Output and Airplane toggle on click; the Network tile only opens its page, and Airplane has no page, so no chevron. The bottom row ends with a button for the COSMIC Settings app, and Applet settings… below it opens this applet's own settings."),
    screen("m1", "1. Main", "Battery on top, sliders in the middle, tiles for the switchable parts, power buttons at the bottom.", main_page),
    screen("m2", "2. Low battery", "Below 10% the header turns red, and a banner explains what changed automatically.", low),
    screen("m3", "3. Airplane mode, muted", "Off tiles stay in place and only lose their accent colour, so the layout never jumps.", airplane),
    screen("m4", "4. Ethernet + Wi-Fi + VPN", "The Network tile always shows the connection in use: Ethernet, plus the VPN. Wi-Fi on/off lives on the Network page.", both),
    screen("m5", "5. Desktop without Wi-Fi or battery", "Sections without hardware are left out. Without a Wi-Fi card the Network tile shows Ethernet, and a VPN tile takes the free slot.", desktop),
    heading("Detail pages",
            "One page per chevron, all built the same way: ← back to the main popup, the area's switch if it has one, then devices and options, then a link to the matching page of the COSMIC Settings app."),
    screen("d1", "6. Battery", "From the battery header. Power profiles from power-profiles-daemon, the charge limit if the hardware supports it, and peripheral batteries from UPower.", b_power),
    screen("d2", "7. Sound", "From the Output tile. Output and input devices, each with its own slider. Picking a device switches the default sink or source.", b_audio),
    screen("d3", "8. Network", "From the Network tile. Three sections: Wired (only with an Ethernet port), Wi-Fi with its radio switch and scan results, and VPN profiles with one switch each.", b_network),
    screen("d3b", "9. Network: Ethernet + Wi-Fi + VPN", "Both links stay connected. \u201cIn use\u201d marks the default route, so Wi-Fi takes over if the cable is pulled. A connected VPN says which link it runs over.", b_network_both),
    screen("d4", "10. Bluetooth", "From the Bluetooth tile. Paired devices with battery levels, then a live scan. Pairing opens a confirmation when a PIN is needed.", b_bt),
    heading("Other", "Shared pieces that aren't tied to one area."),
    screen("o1", "11. Shut down confirmation", "Restart and Shut down ask first with a 60 s countdown. Lock and Suspend act immediately; the gear opens the COSMIC Settings app.", confirm),
    screen("o2", "12. Settings", "From Applet settings… at the bottom of the main popup, or by right-clicking the panel icon. Pick and reorder popup sections and pick the panel icons. Alerts: no network, low battery, muted output, a Bluetooth device with low battery, a VPN that dropped unexpectedly, and a muted microphone while an app is recording.", settings),
])

CSS = BASE_CSS + """
.wide.sh{margin-top:6px}.sh h3{font-size:17px;line-height:24px;margin:0;font-weight:600}.sh p{font-size:13px;color:var(--mut);margin:2px 0 -10px}
.tag{margin-left:8px;font-size:11px;font-weight:600;padding:1px 7px;border-radius:9px;background:var(--btn)}
.ap .i+.i{margin-left:-2px}.ap.alert{color:var(--bad)}
.i.w24{width:24px;height:24px;-webkit-mask-size:24px;mask-size:24px}
.bh{display:flex;align-items:center;gap:12px;padding:6px 16px 8px}.bh .t{flex:1;min-width:0}
.big2{font-size:20px;font-weight:600;font-variant-numeric:tabular-nums}
.bh2{display:flex;align-items:baseline;gap:10px}
.meter{height:4px;border-radius:2px;background:var(--btn);margin-top:6px;overflow:hidden}.meter b{display:block;height:100%;background:var(--acc)}
.meter.lg{height:8px;border-radius:4px;margin-top:0}
.sl{display:flex;align-items:center;gap:8px;padding:2px 16px 2px 10px;min-height:40px}
.track{flex:1;height:4px;border-radius:2px;background:var(--btn);position:relative}
.track b{position:absolute;left:0;top:0;bottom:0;border-radius:2px;background:var(--acc)}
.track i{position:absolute;top:50%;width:16px;height:16px;margin:-8px 0 0 -8px;border-radius:50%;background:var(--acc)}
.sl.mute .track b,.sl.mute .track i{background:var(--mut)}
.pct{min-width:36px;text-align:right;font-size:13px}
.tiles{display:grid;grid-template-columns:1fr 1fr;gap:8px;padding:8px 12px 6px}
.tile{display:flex;align-items:center;gap:10px;padding:8px 6px 8px 12px;border-radius:12px;background:var(--btn);min-height:52px}
.tile .t{flex:1;min-width:0}.tile .n{font-weight:600;font-size:13px;line-height:17px}.tile .c{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.tile.on{background:var(--acc);color:var(--onacc)}.tile.on .c{color:var(--onacc);opacity:.75}
.tile .more{width:24px;height:36px;display:flex;align-items:center;justify-content:center;border-left:1px solid rgba(127,127,127,.35);padding-left:4px}
.inset{margin:4px 12px 6px;border-radius:12px;background:var(--pri);padding:2px 0 4px}
.inset .row{padding:4px 12px;min-height:40px}
.gh.flex{display:flex;align-items:center;justify-content:space-between;padding-right:16px}.inset .gh.flex{padding:8px 12px 2px}
.pw{display:flex;justify-content:space-between;padding:6px 16px 4px}
.pb{width:44px;height:36px;border-radius:18px;background:var(--btn);display:inline-flex;align-items:center;justify-content:center}
.pb.on{background:var(--bad);color:var(--onacc)}
.sec{min-height:48px}.ch .mk{width:16px;display:inline-flex;color:var(--acc)}
.link.sm{font-size:12px}
.chip .i{width:12px}
.acts.mid{justify-content:center;margin-top:16px}.blank .acts .btn{margin-top:0}
.bh.low>.i,.bh.low .big2{color:var(--bad)}.bh.low .meter b{background:var(--bad)}
.pick .i{flex:none}
.bh .prof{margin-right:-4px}.bh .more{width:28px;height:36px;display:flex;align-items:center;justify-content:center;border-left:1px solid var(--div);margin-left:4px}
.sect{display:flex;align-items:center;justify-content:space-between;padding:6px 16px 2px;font-weight:600;min-height:36px}
.gh2{font-size:12px;font-weight:600;color:var(--mut);padding:6px 16px 0}
.pill{font-size:11px;font-weight:600;padding:1px 8px;border-radius:9px;background:var(--acc);color:var(--onacc)}
.bars{row-gap:20px}
.pw{padding:6px 14px 4px}.pb{width:42px}
.applet{min-height:36px;padding-top:2px}
.btn.danger{background:var(--bad);color:var(--onacc)}
"""

HTML = f"""<!doctype html>
<html lang="en" data-theme="dark"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>System applet mockups</title>
<script>document.documentElement.dataset.theme=new URLSearchParams(location.search).get('theme')||'dark'</script>
<style>{CSS}</style></head><body>
<h1>System applet mockups</h1>
<div class="lead">One applet for audio, battery, Bluetooth, network and power. Static sample renderings with invented data, using COSMIC theme colours. Add ?theme=light for the light theme.</div>
<main>{PANEL}{SCREENS}</main>
</body></html>
"""
open(OUT, "w").write(HTML)
print(len(HTML))

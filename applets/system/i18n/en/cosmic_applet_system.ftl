app-title = System
app-comment = Audio, battery, Bluetooth, network and power in one applet
app-keywords = Volume;Sound;Battery;Power;Bluetooth;Network;Wi-Fi;VPN;Shutdown;

# Pages and tiles
battery = Battery
sound = Sound
network = Network
bluetooth = Bluetooth
settings = Settings
wifi = Wi-Fi
output = Output
not-available = Not available yet.
applet-settings = Applet settings…
applet-settings-title = Applet settings
cosmic-settings = COSMIC Settings
power-settings = Power settings…
sound-settings = Sound settings…
network-settings = Network settings…
bluetooth-settings = Bluetooth settings…

# Power
lock = Lock
log-out = Log out
suspend = Suspend
restart = Restart
shutdown = Shut down
cancel = Cancel
confirm-shutdown = Shut down?
confirm-shutdown-body = The computer shuts down automatically in { $seconds } seconds.
shutdown-now = Shut down now
confirm-restart = Restart?
confirm-restart-body = The computer restarts automatically in { $seconds } seconds.
restart-now = Restart now
confirm-log-out = Log out?
confirm-log-out-body = You are logged out automatically in { $seconds } seconds.
log-out-now = Log out now

# Sound
input = Input
muted = Muted
no-output = No output
audio-unavailable = The audio service (cosmic-settings-daemon) isn't running.

# Battery
power-mode = Power mode
time-left = { $hours ->
    [0] { $minutes } min left
   *[other] { $hours } h { $minutes } min left
}
until-full = { $hours ->
    [0] { $minutes } min until full
   *[other] { $hours } h { $minutes } min until full
}
fully-charged = Fully charged
not-charging = Plugged in, not charging
charging = Charging
on-battery = On battery
battery-low = Battery low
charge-limit = Limit charge to { $percent }%
charge-limit-description = Protects the battery when mostly plugged in
connected-devices = Connected devices
profile-power-saver = Battery saver
profile-balanced = Balanced
profile-performance = Performance
profile-power-saver-description = Lower performance, longer runtime
profile-balanced-description = Standard performance and power use
profile-performance-description = Higher performance and power use

# Bluetooth
on = On
off = Off
devices-connected = { $count } connected
my-devices = My devices
nearby = Nearby
searching = Searching…
pair = Pair
connect = Connect
disconnect = Disconnect
connected = Connected
connected-battery = Connected · { $percent }%
not-connected = Not connected
bluetooth-unavailable = No Bluetooth adapter found.

# Network
wired = Wired
ethernet = Ethernet
vpn = VPN
airplane = Airplane
in-use = In use
disconnected = Disconnected
connecting = Connecting…
connected-speed = Connected · { $speed }
connected-standby = Connected · standby
connected-via = Connected via { $link }
cable-unplugged = Cable unplugged
other-networks = Other networks
open-network = Open network
password = Password
network-unavailable = NetworkManager isn't running.

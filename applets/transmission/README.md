# COSMIC Transmission applet

Panel applet for a remote [Transmission](https://transmissionbt.com/) daemon, e.g. on a NAS.
The panel shows a ring with the overall progress of the running downloads and their count
inside; the popup is a read-only list of torrents (Active / Downloading / Seeding / All).
Clicking the list opens your Transmission client.

- Needs Transmission **4.1 or newer**: it speaks the JSON-RPC 2.0 protocol introduced in 4.1.
  Plain HTTP only (no TLS), meant for the local network.
- Set up the connection with the gear button in the popup. The password is stored in the
  desktop keyring (Secret Service, e.g. gnome-keyring, viewable in *Passwords and Keys*), keyed
  by `host:port` and username. The login keyring is unlocked at login; if it is locked, the
  keyring shows its own unlock prompt once.
- The ring shows `!` when a torrent failed and a dashed ring when the server is unreachable or
  the login failed.
- Notifications when a download finishes or a torrent fails (both can be turned off).
- The client command defaults to `flatpak run io.github.TransmissionRemoteGtk`.

Config lives in `~/.config/cosmic/at.hoeggi.CosmicAppletTransmission/v1/`
(`host`, `port`, `rpc_path`, `username`, `client_command`, `poll_secs`, …).

## Development

Build, run and package from the repository root (see the [top-level README](../../README.md)):

    make run APPLET=transmission

The RPC client has an opt-in test against a real daemon (run inside `make shell`):

    TRANSMISSION_URL=http://nas:9091/transmission/rpc TRANSMISSION_USER=… TRANSMISSION_PASSWORD=… \
      cargo test -p cosmic-applet-transmission -- --ignored --nocapture live_server

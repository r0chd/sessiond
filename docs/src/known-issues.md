# Known issues

This page lists known issues with third-party software and their workarounds.

## ly

When `sessiond` is built with the `consolekit` feature, it does not set `XDG_SESSION_ID` in the session environment.

[ly](https://codeberg.org/fairyglade/ly) sets `XDG_SESSION_ID` to `1` for every session it starts. Because `sessiond`
leaves the variable unset, this value is passed through to the session unchanged. The ID does not match the session
that `sessiond` registered, so polkit authentication agents that read `XDG_SESSION_ID` register for the wrong session.
They then refuse to open an authentication dialog.

**Workaround**: Unset `XDG_SESSION_ID` before starting the polkit agent, for example in your compositor's startup
script:

```sh
unset XDG_SESSION_ID
```

## Applications that assume systemd-logind

On systems where `/run/systemd` exists, such as [NixOS](installation/nixos.md), some applications assume that
`systemd-logind` is available. They check `/run/systemd/sessions` to determine whether the current session is active.
`sessiond` does not populate this directory, so these applications may behave incorrectly.

**General solution**: If the application can be built without logind support, build it that way.

### WirePlumber

WirePlumber's Bluetooth monitor may consider the seat inactive and never enable the Bluetooth monitor.

**Workaround**: Disable seat monitoring for the Bluetooth monitor using the `monitor.bluez.seat-monitoring` profile
setting, for example in `~/.config/wireplumber/wireplumber.conf.d/99-no-seat-monitoring.conf`:

```text
wireplumber.profiles = {
  main = {
    monitor.bluez.seat-monitoring = disabled
  }
}
```

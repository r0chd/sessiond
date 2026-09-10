# sessiond-power

`sessiond-power` provides authorized reboot, poweroff, suspend, hibernate, and
hybrid-sleep actions.

## Actions

The action commands are configured independently, for example:

```toml
reboot = ["/usr/bin/reboot"]
poweroff = ["/usr/bin/poweroff"]
hibernate = ["/usr/bin/zzz", "-Z"]
suspend = ["/usr/bin/zzz", "-z"]
hybrid-sleep = ["/usr/bin/zzz", "-H"]
```

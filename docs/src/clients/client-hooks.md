# sessiond-hooks

`sessiond-hooks` launches and supervises a per-user session hook for every
unique UID observed by `sessiond`. It can be used to start a per-user service
manager such as `dinit --user` or `runsvdir`.

## Configuration

```toml
on_start = "/usr/bin/dinit"

[restart]
on = "Failure"
delay = 5
```

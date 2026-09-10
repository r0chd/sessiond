# sessiond-uaccess

`sessiond-uaccess` grants device-node access to active local sessions by
writing POSIX ACL entries.

## Rules

Rules are Lua files loaded from the configured rules directories:

```lua
device {
  access = "rw",
  match = function(dev)
    return dev.subsystem == "sound"
  end,
}
```

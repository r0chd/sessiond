# Clients

`sessiond` provides session registration, tracking, and querying. Additional
system integration is implemented by clients.

Known clients:

- [`sessiond-hooks`](client-hooks.md): launches and supervises per-user session
  hooks.
- [`sessiond-backlight`](client-backlight.md): controls backlight and LED
  brightness for active local sessions.
- [`sessiond-uaccess`](client-uaccess.md): manages dynamic device access rules.
- [`sessiond-power`](client-power.md): handles authorized system power actions.

# Known Issues

This document tracks known compatibility issues between `sessiond` and other
applications.

## Noctalia polkit agent

**Symptom:** the noctalia polkit agent never starts under `sessiond`.

**Cause:** for performance reasons, noctalia first compares `XDG_SESSION_ID` against the session
id returned by the login1 `GetSessionForUnixProcess` method, only falling back to a slower
backend-independent lookup if that fails. With the `ConsoleKit` backend, `GetSessionForUnixProcess`
returns an object path rather than the integer noctalia expects, so this fast path is meant to be
skipped which normally happens because `ConsoleKit` doesn't set `XDG_SESSION_ID` at all. `sessiond`
does set it, so noctalia takes the fast path, gets a mismatch, and exits early instead of falling
back.

**Workaround:** unset the `XDG_SESSION_ID` environment variable in noctalia's service file, or in
your shell before launching it from the CLI.

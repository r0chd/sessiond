# ConsoleKit interface

When built with the `consolekit` feature, `sessiond` provides a partial
implementation of the `org.freedesktop.ConsoleKit` D-Bus interface. This
allows software that still uses ConsoleKit's session API to discover sessions
registered with `sessiond`.

The manager object is available at:

```text
org.freedesktop.ConsoleKit
/org/freedesktop/ConsoleKit/Manager
```

Supported manager methods:

- `GetSessionForUnixProcess`
- `GetCurrentSession`
- `GetSessions`

Supported session methods:

- `GetUnixUser`
- `GetUser`
- `IsActive`
- `IsLocal`
- `GetId`
- `GetLoginSessionId`
- `GetCreationTime`

Supported session properties:

- `unix-user`
- `is-active`
- `is-local`

This is a compatibility interface rather than a complete ConsoleKit implementation. It provides a drop-in
interface for Polkit to use sessiond as a session information provider. If Polkit gains native sessiond support,
this interface will be deprecated and removed.

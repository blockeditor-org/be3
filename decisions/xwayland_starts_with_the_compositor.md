# Xwayland starts with the compositor, not on demand

be-host starts Xwayland as soon as its Wayland server is up (and retries before every launch),
rather than when an X11 client first connects.

## Why

smithay creates the Xwayland client and its X11 sockets only inside `XWayland::spawn`, which
starts the server at once, so there is no socket to listen on before the server runs.

## Rejected

- Starting Xwayland lazily on the first X11 connection: smithay offers no way to own the X11
  sockets without starting the server.

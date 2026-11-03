# The account portal's beui pages are linked only into be-server's executable

be-server decides what each `/auth` request shows as a `be_server_portal::Page`
and renders it through the `Portal` in `ServerConfig::portal`. Only the
executable, crates/be-server-bin, passes crates/be-server-portal-ui, the beui
renderer; the be-server library, its tests and every server embedded in a
native program run with no portal and answer each page with its `Page` as JSON.

## Why

- The be-server library and its tests build on the native core and never wait
  on beui, which keeps them fast to build.
- Embedded servers keep the same routes, redirects, cookies and form checks, so
  tests of the portal's behaviour need no renderer.

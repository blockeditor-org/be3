# Common commands

The commands most runs of the app need, beside those AGENTS.md lists. guides/buck2.md has every
target; guides/running/native.md, web.md, android.md and drive.md say how to run and drive each.

```
# the app in a window
./scripts/buck run //crates/block-app:native

# build the APK, install it on the connected device and start it
./scripts/buck run //crates/block-app:android -- --install

# beui's demo in a window
./scripts/buck run //crates/beui-demo:native

# the desktop shell in a window
./scripts/buck run //crates/block-app:native -- --desktop

# the desktop as a login session on this machine's displays, from a virtual terminal with no
# other display server on it (buck2 run passes the environment on to the app)
BEUI_PLATFORM=drm ./scripts/buck run //crates/block-app:native -- --desktop

# install it as a login session the login screen offers, under /usr/local or PREFIX
./scripts/buck run //crates/block-app:install-login-session [-- PREFIX]
```

The login session and its install are Linux's alone: guides/desktop/linux.md.

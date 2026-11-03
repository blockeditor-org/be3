# be-host starts apps before its window and maps their compiled code

be-host warms the apps a restored workspace will need as soon as it knows it will open a
workspace, before its window and GPU are set up, and on desktop maps every `.cwasm` from its
file instead of reading it into memory (guides/performance.md, "Starting be-host").

## Why

Measured locally in October 2026:

- Starting workspace-ui and the file tree together, before the window, took the time to the
  Files pane from about 0.92 s to 0.76 s. Most of an app's start is linking its libraries,
  which can run on the app's own thread while the host sets up.
- Mapping the `.cwasm` files (about 400 MB for the host's apps, libraries and type modules)
  took about a third of a second off the time to the Files pane.

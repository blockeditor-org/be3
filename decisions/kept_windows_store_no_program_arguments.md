# Kept windows record a program's executable, never its arguments

When the Linux host fills `HostWindow::origin` for a window it did not launch, it records the
program's executable (the first word of the client's `/proc` command line), never the rest of the
command line. Placeholders for windows kept across restarts are opened from that.

## Why

Saved layouts outlive the process, and an argument can be a secret.

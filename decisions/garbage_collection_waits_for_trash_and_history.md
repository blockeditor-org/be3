# The server collects no garbage until trash and history say what may go

The host never sends `CollectDetached` or `PruneHistory`, and objects uploaded but never published are never collected. Collection stays off until trash and history define what may be reclaimed, and any automatic purge of the trash waits until history and backups are trusted.

## Why

Collecting before then could delete what a user still expects to restore. A conflicted merge's kept side (`Peer::stash`) is not yet counted as held by the server, so it must be held before any collection runs.

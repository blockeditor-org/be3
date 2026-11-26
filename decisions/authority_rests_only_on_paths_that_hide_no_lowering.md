# Authority rests only on paths that hide no lowering

A record signed on authority from another log (`Authority::Inherited`, or the
organization's) is checked against that log as the reader is served it, which
for a reader who may not view it is a proof path. `be_block_keys::answer` takes
no authority from a path that reduces an entry that lowered anyone
(`Checked::hides_lowering`), nor from a block's path that reduces a retirement
(`Reduced::retires`, committed by the entry's hash, `Checked::hides_retirement`).
So that honest paths still serve, the server keeps whole, on any path of a
block's log that records below rest their authority on, every entry that
lowered anyone, every retirement, and the accounts they name (`proof_path`'s
`lowerings`). The flag is per log, so every reader served a path of that log
sees them. It is set while `access_log_authorities` names the log as above a
record, which the append of the first such record below writes, so it holds
from the first path served after that record. The organization's paths already
reduce no lowering and no administrator's retirement (every record about an
administrator is whole), and one that hides a lowering names no administrator.

## Why

A reduced entry hides whom it is about. A server could reduce a manager's
revocation on the path it serves a reader below, and the reader would still
take a grant that manager signed after it; or reduce a manager's retirement, and
the reader would take a grant signed with the retired key, which may be the
one stolen. A lowering or retirement kept whole closes that, and refusing a
path that hides one makes a server that reduces it anyway fail closed.

The price, accepted: a reader below a block learns every revocation, downgrade
and retirement on it, and the level history of each account they name, though
not the grants that lowered no one.

## Rejected

- **Refusing every partial log as authority.** A reader granted from above,
  which may not view the block above, would fail to check its own block's log,
  unless it were served whole logs of the blocks above, which tells it every
  grant there (decisions/editors_are_served_whole_access_logs.md).
- **Refusing a path that hides a lowering, with paths left as they were.**
  Sound, but any revocation on a block above would leave every reader granted
  from it unable to check its log.
- **A reduced step that commits whom it lowered**, or from what level. A wider
  change to the step's hash for the same protection, which still tells the
  reader that a manager was lowered.

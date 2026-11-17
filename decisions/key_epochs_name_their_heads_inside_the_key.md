# Key epochs name their heads inside the key

Each new scope or data key names the heads of the access logs it was made
under (its scope's block log and the organization's), inside the key itself
(`CarriedKey::heads`): sealed or wrapped by its maker and passed on unchanged
by every sender after. A device seals an epoch only to whom the logs grant as
of a head at or after those, and writes under it no more once an entry that
lowered anyone follows them (guides/blocks/keys.md, "Epochs name their
heads"). An escrow epoch names the organization's head in its signed record
(`EscrowKey::head`), which the server takes only at its head now.

## Why

A device knows a log only as far as the server shows it. If a revocation is
withheld from a device doing key work, it would seal the next epoch to the
revoked member. A revocation is always followed by a rotation whose epoch
names a head after it, so no device that holds that epoch seals it to them.
Heads inside the key are as good as the key's own sender, and no one who
never held the key can name other heads for it.

## Rejected

- **A signature on the key row beside the key.** A revoked member, with the
  server, could swap it for one of their own naming an older head.
- **Rotating each outdated scope from below.** A scope's new epoch is wrapped
  under a new one of the scope above, so rotating from below would rotate the
  scopes above again at each level; a rotation starts from the highest
  outdated scope instead (`Peer::rotation_top`).

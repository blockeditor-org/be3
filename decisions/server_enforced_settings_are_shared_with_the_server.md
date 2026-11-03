# A setting the server enforces is shared with the server

The server has an account of its own ("This server", `be-server/src/organization.rs`,
`ServerKey`). The organization settings block is shared with it
(`Peer::share_with_server`, View only), the server reads its policies, and it
refuses what a policy governs while it can't read the policy
(`ErrorCode::PolicyUnreadable`).

## Why

The server can enforce only a policy it can read, so it is given the
policy's block like any other reader, and an action whose policy it can't read
is refused rather than allowed.

## Rejected

- Storing the setting unencrypted: only acceptable for data that doesn't matter.

# Workspace keys are kept by each device, with no device key pairs

Each device keeps a workspace's random key itself. The server holds only copies sealed to recovery keys: a recovery phrase per account whose public half seals every workspace key, used to unlock a new device with the phrase or with a code typed on an open device (SPAKE2 over the server's relay), and sealing for invited members. guides/the_new_block_stack.md (Keys) describes it.

## Why

A device key pair added nothing a device's own copy of the key does not already give it, and pairing hands the key over directly.

## Rejected

- A key pair per device, with workspace keys sealed to each device, as first designed.

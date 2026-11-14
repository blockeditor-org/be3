# Each server is a NixOS configuration in the repository

Every server is a NixOS configuration in `servers/` (guides/servers.md), installed with nixos-anywhere and changed only
by deploying it, so the repository is the source of truth for what runs where. Data lives on each server's volume.

## Why

The API server's unit, proxy and data directory were set up by hand on the host, so nobody could tell what was on it
or rebuild it. A NixOS configuration is the whole machine: what is not in it does not exist there, a deploy cannot
leave the machine half changed, and a bad one rolls back.

## Rejected

- Terraform or OpenTofu: it manages the provider's resources (servers, firewalls, DNS), which change a few times a
  year, and not what runs on the machines, which is where the setup is.
- A setup script per server (as the build server's `scripts/internal/nativelink/setup.sh`): it says what to change
  rather than what the machine is, so anything changed by hand stays, and a rerun restarts everything.
- Ansible: the same, with another language.

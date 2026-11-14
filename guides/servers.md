# Servers

`servers/` is a Nix flake with a NixOS configuration for each server: everything on the machine (packages, systemd
units, Caddy, the firewall, users and SSH keys) is built from it, and a deploy replaces whatever was there. The
machine keeps only data, on its attached volume. Changing a server is changing its configuration here and deploying
it; nothing is changed by hand on the machine, which the next deploy would undo.

| Configuration | Machine | What it runs |
| --- | --- | --- |
| `api` (`servers/api`) | `api.be3.pfg.pw`: Hetzner CX23, Helsinki, `65.21.242.138` and `2a01:4f9:c01d:9e6::1`, a 100 GB volume at `/mnt/data` | be-server behind Caddy's gateway (guides/hosting.md) |

`servers/common.nix` is what every server has: key-only SSH as root for the keys it lists, a firewall with only SSH
open (each server opens what it serves) and a weekly Nix garbage collection.

Outside the configuration, in Hetzner's console: each server's firewall allows inbound TCP 22, 80 and 443, UDP 443
and ICMP, and the DNS records name its addresses.

## Deploying a change

From any machine with Nix (flakes enabled) and the agents' SSH key:

```
nix run nixpkgs#nixos-rebuild -- switch --flake ./servers#api --target-host root@api.be3.pfg.pw
```

It builds the system here, copies it over and switches to it; `--build-host root@api.be3.pfg.pw` builds there
instead. A bad switch is undone with `nixos-rebuild switch --rollback` on the machine, or by picking the previous
generation in GRUB. `nix flake update --flake ./servers` moves nixpkgs forward.

be-server itself is not part of the configuration: `./scripts/deploy` copies the binary and its `build-info.json` to
`/srv/be-server` and restarts the unit, which runs it through `programs.nix-ld` (buck2 builds it for any Linux).

## A new machine

nixos-anywhere installs a configuration over SSH onto a server running any Linux, partitioning its own disk as the
configuration's `disk.nix` says (never the volume) and keeping its SSH host keys:

```
nix run github:nix-community/nixos-anywhere/1.13.0 -- --flake ./servers#api \
  --target-host root@ADDRESS -i ~/.ssh/hetzner_be3 --copy-host-keys
```

Give the configuration the new machine's disk and volume ids (`ls -l /dev/disk/by-id`) and IPv6 address first.
nixos-anywhere boots an installer with kexec, which Ubuntu 26.04's kernel refused through `kexec_file_load`
("Address not available"). Then edit `/root/kexec/kexec/run` on the machine to pass `--kexec-syscall` rather than
`--kexec-syscall-auto`, run it there, wait for the installer's SSH, and finish with
`--phases disko,install,reboot`.

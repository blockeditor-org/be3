# Servers

`servers/` is a Nix flake with a NixOS configuration for each server: everything on the machine (packages, systemd
units, Caddy, the firewall, users and SSH keys) is built from it, and a deploy replaces whatever was there. The
machine keeps only data: on its attached volume, or on its own disk where it has none. Changing a server is changing
its configuration here and deploying it; nothing is changed by hand on the machine, which the next deploy would undo.

| Configuration | Machine | What it runs |
| --- | --- | --- |
| `api` (`servers/api`) | `api.be3.pfg.pw`: Hetzner CX23, Helsinki, `65.21.242.138` and `2a01:4f9:c01d:9e6::1`, a 100 GB volume at `/mnt/data` | be-server behind Caddy's gateway (guides/hosting.md) |
| `build` (`servers/build`) | `nl.be3.pfg.pw`: Hetzner CPX62, Helsinki, `95.216.205.46` and `2a01:4f9:c015:fbaf::1`, no volume: the cache, the dashboard's history and Caddy's tokens are under `/var/lib` | NativeLink, Caddy and the dashboard (guides/build_server.md) |

`servers/common.nix` is what every server has and shares. Its `module`, which every configuration imports, is key-only
SSH as root for the keys it lists, a firewall with only SSH open (each server opens what it serves) and a weekly Nix
garbage collection. Its `disk` is the partitioning of a server's own disk, given the disk's id, and its `gate` is the
page Caddy serves a browser without the server's secret, which sets the cookie Caddy checks, given its labels, the
cookie's name and where it goes next.

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

A change is checked short of deploying by evaluating it, such as
`nix eval .#nixosConfigurations.build.config.system.build.toplevel.drvPath` in `servers/`, and comparing what it
names before and after (`nix derivation show -r`). In a git worktree Nix cannot read the git objects, so evaluate a
copy of the repository as `path:COPY?dir=servers#...` instead; a plain `path:` of servers/ alone leaves out the
re-relay source the build server takes from scripts/.

be-server itself is not part of the configuration: `./scripts/deploy` copies the binary and its `build-info.json` to
`/srv/be-server` and restarts the unit, which runs it through `programs.nix-ld` (buck2 builds it for any Linux).

## A new machine

nixos-anywhere installs a configuration over SSH onto a server running any Linux, partitioning its own disk as the
configuration's call to `common.disk` says (never the volume) and keeping its SSH host keys:

```
nix run github:nix-community/nixos-anywhere/1.13.0 -- --flake ./servers#api \
  --target-host root@ADDRESS -i ~/.ssh/hetzner_be3 --copy-host-keys
```

Give the configuration the new machine's disk and volume ids (`ls -l /dev/disk/by-id`) and IPv6 address first.
nixos-anywhere boots an installer with kexec, which Ubuntu 26.04's kernel refused through `kexec_file_load`
("Address not available"). Then edit `/root/kexec/kexec/run` on the machine to pass `--kexec-syscall` rather than
`--kexec-syscall-auto`, run it there, wait for the installer's SSH, and finish with
`--phases disko,install,reboot`.

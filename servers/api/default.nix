# api.be3.pfg.pw: be-server behind Caddy, on a Hetzner CX23 in Helsinki (guides/servers.md, guides/hosting.md).
#
# be-server listens on 127.0.0.1 only. Caddy (Caddyfile) is the only way in from outside, and passes a request on
# only with the gateway key, as the Be3-Gateway-Authorization header or the cookie its page sets: the server is not
# yet trusted to face the internet by itself. Everything the server keeps is on the attached volume, /mnt/data.
{ modulesPath, pkgs, ... }:
let
  host = "api.be3.pfg.pw";
  data = "/mnt/data";
  # ./scripts/deploy copies be-server and its build-info.json here (crates/be-tool-local's deploy.rs).
  program = "/srv/be-server";
  site = pkgs.runCommand "api-gate" { } "mkdir $out && cp ${./gate.html} $out/gate.html";
in
{
  imports = [
    (modulesPath + "/profiles/qemu-guest.nix")
    ./disk.nix
  ];

  networking.hostName = "api";
  system.stateVersion = "26.05";

  boot.loader.grub = {
    enable = true;
    efiSupport = true;
    efiInstallAsRemovable = true;
  };
  boot.initrd.availableKernelModules = [ "ata_piix" "uhci_hcd" "virtio_pci" "virtio_scsi" "sd_mod" "sr_mod" ];

  # Hetzner gives the IPv4 address by DHCP and the IPv6 one only in its console.
  networking.useDHCP = false;
  networking.useNetworkd = true;
  systemd.network.networks."10-wan" = {
    matchConfig.Name = "eth0 en*";
    networkConfig.DHCP = "ipv4";
    address = [ "2a01:4f9:c01d:9e6::1/64" ];
    routes = [ { Gateway = "fe80::1"; } ];
  };

  # The volume: be-server's data and the gateway key. nofail, so a missing volume leaves the machine reachable.
  fileSystems.${data} = {
    device = "/dev/disk/by-id/scsi-0HC_Volume_107464334";
    fsType = "ext4";
    options = [ "nofail" "x-systemd.device-timeout=30s" ];
  };

  swapDevices = [ { device = "/swapfile"; size = 2048; } ];

  networking.firewall = {
    allowedTCPPorts = [ 80 443 ];
    allowedUDPPorts = [ 443 ];
  };

  # be-server is built by buck2 for any Linux, so it asks for /lib64/ld-linux-x86-64.so.2, glibc and libgcc_s.
  programs.nix-ld.enable = true;

  users.users.be-server = {
    isSystemUser = true;
    group = "be-server";
  };
  users.groups.be-server = { };

  # The directories on the volume, and the gateway key, made once and kept there. Its value is what clients send:
  # `ssh root@api.be3.pfg.pw cat /mnt/data/secrets/gateway.env`.
  systemd.services.api-data = {
    description = "be-server's directories and the gateway key on the volume";
    unitConfig.RequiresMountsFor = data;
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
    };
    script = ''
      install -d -o be-server -g be-server -m 0750 ${data}/be-server
      install -d -m 0700 ${data}/secrets
      if [ ! -s ${data}/secrets/gateway.env ]; then
        (umask 077; echo "API_GATEWAY_KEY=$(head -c 32 /dev/urandom | ${pkgs.coreutils}/bin/base64 | tr -dc 'A-Za-z0-9')" > ${data}/secrets/gateway.env)
      fi
    '';
  };

  systemd.tmpfiles.rules = [ "d ${program} 0755 root root -" ];

  systemd.services.be-server = {
    description = "be-server, behind Caddy";
    after = [ "network-online.target" "api-data.service" ];
    wants = [ "network-online.target" ];
    requires = [ "api-data.service" ];
    wantedBy = [ "multi-user.target" ];
    unitConfig = {
      RequiresMountsFor = data;
      # Nothing to run until ./scripts/deploy has put it there.
      ConditionPathExists = "${program}/be-server";
    };
    environment.NIX_LD_LIBRARY_PATH = "/run/current-system/sw/share/nix-ld/lib";
    serviceConfig = {
      User = "be-server";
      Group = "be-server";
      ExecStart = builtins.concatStringsSep " " [
        "${program}/be-server"
        "--addr 127.0.0.1:9090"
        "--data-dir ${data}/be-server"
        "--public-url https://${host}/api/be"
        "--auth-origin https://${host}"
        "--allow-login"
        # Registering needs an invite code too (guides/hosting.md, Accounts and the portal).
        "--allow-registration"
      ];
      Restart = "always";
      RestartSec = 3;
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [ "${data}/be-server" ];
      PrivateTmp = true;
      NoNewPrivileges = true;
    };
  };

  services.caddy = {
    enable = true;
    configFile = ./Caddyfile;
  };
  systemd.services.caddy = {
    after = [ "api-data.service" ];
    requires = [ "api-data.service" ];
    environment = {
      API_HOST = host;
      API_SITE = "${site}";
    };
    serviceConfig.EnvironmentFile = "${data}/secrets/gateway.env";
  };

  environment.systemPackages = [ pkgs.sqlite pkgs.htop ];
}

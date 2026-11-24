# What every server shares (guides/servers.md): `module`, which every configuration imports, and `disk` and `gate`,
# which a server calls with what is its own.
{
  # Key-only SSH for the people and agents that run the servers, a firewall with nothing open but SSH (each server
  # opens what it serves), and Nix kept tidy.
  module = { ... }: {
    services.openssh = {
      enable = true;
      settings = {
        PasswordAuthentication = false;
        KbdInteractiveAuthentication = false;
        PermitRootLogin = "prohibit-password";
      };
    };
    users.users.root.openssh.authorizedKeys.keys = [
      # Agents' key (~/.ssh/hetzner_be3 on the machines they deploy from).
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDL79C5G2D0ONHfw/mPQudf0hY2EkD7boM6LJzq9gesc be3-hetzner-test"
      # The owner's.
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIE8XE24zrQ5Z5cBfeP3ZJeYjlY6e3vIfK2VU78G9uBXU"
      # CI deploy key: the Deploy workflow's (.github/workflows/deploy.yml, guides/hosting.md, Keys), a key of its
      # own whose private half is the deploy environment's BE3_DEPLOY_SSH_KEY. Uncomment it with the public key.
      # "ssh-ed25519 AAAA... be3-ci-deploy"
    ];

    networking.firewall = {
      enable = true;
      allowedTCPPorts = [ 22 ];
    };

    nix = {
      settings.experimental-features = [ "nix-command" "flakes" ];
      gc = {
        automatic = true;
        dates = "weekly";
        options = "--delete-older-than 30d";
      };
    };

    time.timeZone = "UTC";
  };

  # The server's own disk, which nixos-anywhere partitions when it installs: `id` is its name under /dev/disk/by-id.
  # An attached volume is never touched by this.
  disk = id: {
    disko.devices.disk.main = {
      type = "disk";
      device = "/dev/disk/by-id/${id}";
      content = {
        type = "gpt";
        partitions = {
          # GRUB's partition for booting by BIOS, which Hetzner's cloud servers may boot by, and the ESP for UEFI.
          bios = {
            size = "1M";
            type = "EF02";
          };
          esp = {
            size = "512M";
            type = "EF00";
            content = {
              type = "filesystem";
              format = "vfat";
              mountpoint = "/boot";
              mountOptions = [ "umask=0077" ];
            };
          };
          root = {
            size = "100%";
            content = {
              type = "filesystem";
              format = "ext4";
              mountpoint = "/";
            };
          };
        };
      };
    };
  };

  # A directory holding gate.html, the only page a server's Caddyfile serves to a browser without its secret: it
  # stores what is typed into `field` as the cookie Caddy checks, then runs `next`, lines of JavaScript that go on.
  # `about` is the page's own comment, as lines: what the secret is and where the page goes.
  gate = pkgs: { name, title, about, field, label, button, cookie, next }:
    pkgs.writeTextFile {
      inherit name;
      destination = "/gate.html";
      text = ''
        <!doctype html>
        <html lang="en">
        <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <meta name="robots" content="noindex">
        <title>${title}</title>
        <style>
          :root { color-scheme: light dark; }
          body { font: 16px/1.5 system-ui, sans-serif; margin: 0; padding: 16px; display: grid; place-items: center; min-height: 80vh; }
          form { display: grid; gap: 12px; width: min(100%, 360px); }
          input, button { font: inherit; padding: 8px 10px; }
        </style>
        </head>
        <body>
        <!-- ${builtins.concatStringsSep "\n     " about} -->
        <form id="gate">
          <label for="${field}">${label}</label>
          <input id="${field}" type="password" autocomplete="current-password" required autofocus>
          <button type="submit">${button}</button>
        </form>
        <script>
          document.getElementById("gate").addEventListener("submit", (event) => {
            event.preventDefault();
            const ${field} = document.getElementById("${field}").value.trim();
            document.cookie = "${cookie}=" + encodeURIComponent(${field}) + "; Path=/; Max-Age=31536000; Secure; SameSite=Lax";
            ${builtins.concatStringsSep "\n    " next}
          });
        </script>
        </body>
        </html>
      '';
    };
}

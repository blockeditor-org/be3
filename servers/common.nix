# What every server has: key-only SSH for the people and agents that run them, a firewall with nothing open but SSH
# (each server opens what it serves), and Nix kept tidy.
{ ... }:
{
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
}

# The servers, each a NixOS configuration built from this directory alone (guides/servers.md).
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    disko = {
      url = "github:nix-community/disko/v1.13.0";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, disko, ... }: {
    nixosConfigurations.api = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [ disko.nixosModules.disko ./common.nix ./api ];
    };
  };
}

# The server's own disk, which nixos-anywhere partitions when it installs (guides/servers.md). The attached volume
# (default.nix) is never touched by this.
{
  disko.devices.disk.main = {
    type = "disk";
    device = "/dev/disk/by-id/scsi-0QEMU_QEMU_HARDDISK_133276878";
    content = {
      type = "gpt";
      partitions = {
        # Hetzner's cloud servers boot by BIOS, so GRUB needs this partition; the ESP is there for UEFI too.
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
}

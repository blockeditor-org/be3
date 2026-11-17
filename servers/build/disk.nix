# The server's own disk, which nixos-anywhere partitions when it installs (guides/servers.md). It holds everything,
# the cache included: the build server has no volume.
{
  disko.devices.disk.main = {
    type = "disk";
    device = "/dev/disk/by-id/scsi-0QEMU_QEMU_HARDDISK_133178107";
    content = {
      type = "gpt";
      partitions = {
        # GRUB's partition for booting by BIOS, and the ESP for UEFI, which this server boots by.
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

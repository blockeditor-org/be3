# One action on scripts/local-build-server's worker, run as root in fresh user,
# mount and pid namespaces: the workers' image is the root, with the host's
# devices, a /proc for these processes, a /tmp of the action's own and the
# server's work directory, where the action's inputs are, at the same path.
#
# Usage: build-server-action.sh ROOTFS WORK COMMAND [ARGUMENT...]

set -e
rootfs="$1"
work="$2"
shift 2

mount --rbind /dev "$rootfs/dev"
mount --rbind /sys "$rootfs/sys"
mount -t proc proc "$rootfs/proc"
mount -t tmpfs tmpfs "$rootfs/tmp"
mount --bind "$work" "$rootfs$work"

# NativeLink gives an action only the environment its command names, and a
# container starts with its image's.
export PATH="${PATH:-/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin}"
export HOME="${HOME:-/root}"

# Only root and the user's subordinate groups exist in the namespace, so tar,
# which as root gives what it unpacks the archive's owners, is told not to.
# dpkg-deb asks for them anyway, and its packages are all root's.
export TAR_OPTIONS="--no-same-owner${TAR_OPTIONS:+ $TAR_OPTIONS}"

exec chroot "$rootfs" /bin/sh -c 'cd "$0" && exec "$@"' "$PWD" "$@"

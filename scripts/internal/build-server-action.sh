# One action on scripts/build-server's worker, run as root in fresh user,
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

# The loader variables run-action put aside come back for the action alone.
exec chroot "$rootfs" /bin/sh -c '
    cd "$0" || exit
    for name in LD_LIBRARY_PATH LD_PRELOAD LD_AUDIT; do
        eval "if [ -n \"\${BE3_ACTION_$name+x}\" ]; then export $name=\"\$BE3_ACTION_$name\"; unset BE3_ACTION_$name; fi"
    done
    exec "$@"
' "$PWD" "$@"

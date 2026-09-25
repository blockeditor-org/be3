#!/bin/sh
#
# Signs an APK buck2 built, here, because the key is this machine's: Android
# only installs an update signed with the same key. The keystore is --keystore,
# target/android-debug.keystore by default, made on first use. CI's APK is
# signed on a worker instead (signed_apk). --install installs it with adb and
# starts it.
#
# Usage:
#   ./scripts/buck run //crates/block-app:android -- [--out APK] [--keystore FILE] [--install]
set -eu
jdk="$1" build_tools="$2" platform_tools="$3" unsigned="$4"
shift 4
out=target/android/block-app.apk
keystore=target/android-debug.keystore
install=false
while [ $# -gt 0 ]; do
    case "$1" in
        --out) out="$2"; shift 2 ;;
        --keystore) keystore="$2"; shift 2 ;;
        --install) install=true; shift ;;
        *) echo "Usage: ./scripts/buck run //crates/block-app:android -- [--out APK] [--keystore FILE] [--install]" >&2; exit 1 ;;
    esac
done

if [ ! -e "$keystore" ]; then
    echo "Making a debug keystore at $keystore"
    mkdir -p "$(dirname "$keystore")"
    "$jdk/bin/keytool" -genkeypair -keystore "$keystore" -storepass android -alias androiddebugkey \
        -keypass android -dname 'CN=Android Debug,O=Android,C=US' -keyalg RSA -keysize 2048 -validity 10000 > /dev/null
fi

mkdir -p "$(dirname "$out")"
cp "$unsigned" "$out"
chmod 644 "$out"
JAVA_HOME="$jdk" "$jdk/bin/java" -jar "$build_tools/lib/apksigner.jar" sign --ks "$keystore" --ks-pass pass:android "$out"
echo "Wrote $out"

if $install; then
    package="$("$build_tools/aapt2" dump packagename "$out")"
    "$platform_tools/adb" install -r "$out"
    "$platform_tools/adb" shell am start -n "$package/com.be3.block.MainActivity"
fi

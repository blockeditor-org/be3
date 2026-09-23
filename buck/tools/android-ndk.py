#!/usr/bin/env python3
#
# The part of Android's NDK a build for aarch64-linux-android reads: the
# sysroot's headers and aarch64 libraries, and the compiler runtime clang links
# into an Android program - its builtins and libunwind - laid out as a clang
# resource directory. The NDK is a 780 MB zip of which this is under a hundred;
# the rest is the NDK's own compilers, which the build has in buck/tools.
#
# Usage: android-ndk.py URL SHA256 OUT

import hashlib
import os
import shutil
import sys
import tempfile
import urllib.request
import zipfile

url, sha256, out = sys.argv[1:]

with tempfile.TemporaryFile() as archive:
    digest = hashlib.sha256()
    for attempt in range(5):
        try:
            archive.seek(0)
            archive.truncate()
            digest = hashlib.sha256()
            with urllib.request.urlopen(url, timeout=300) as response:
                while chunk := response.read(1 << 20):
                    digest.update(chunk)
                    archive.write(chunk)
            break
        except Exception:
            if attempt == 4:
                raise
    if digest.hexdigest() != sha256:
        sys.exit("{} has sha256 {}, not {}".format(url, digest.hexdigest(), sha256))

    prebuilt = "android-ndk-r29/toolchains/llvm/prebuilt/linux-x86_64/"
    runtime = prebuilt + "lib/clang/21/"
    with zipfile.ZipFile(archive) as ndk:
        for info in ndk.infolist():
            name = info.filename
            if name.endswith("/"):
                continue
            if name.startswith(prebuilt + "sysroot/usr/include/") or name.startswith(
                prebuilt + "sysroot/usr/lib/aarch64-linux-android/"
            ):
                destination = os.path.join(out, "sysroot", name[len(prebuilt + "sysroot/") :])
            elif name.startswith(runtime + "lib/linux/") and (
                "aarch64-android" in os.path.basename(name) or name.startswith(runtime + "lib/linux/aarch64/lib")
            ):
                destination = os.path.join(out, "clang", name[len(runtime) :])
            else:
                continue
            os.makedirs(os.path.dirname(destination), exist_ok=True)
            mode = info.external_attr >> 16
            if (mode & 0o170000) == 0o120000:
                os.symlink(ndk.read(info).decode(), destination)
                continue
            with ndk.open(info) as source, open(destination, "wb") as target:
                shutil.copyfileobj(source, target)

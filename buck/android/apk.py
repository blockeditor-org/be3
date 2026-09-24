#!/usr/bin/env python3
#
# An unsigned, aligned APK, made the way the Android Gradle plugin makes a debug
# one: aapt2 links the manifest, javac and d8 make classes.dex, the native
# libraries go in stored under lib/arm64-v8a so Android maps them from the APK,
# the assets go under assets/, and zipalign -P 16 puts each library on a 16 KB
# page. Fixed timestamps and order make the same inputs the same bytes. sign.py
# signs it where the key is.
#
# Usage:
#   apk.py OUT --jdk DIR --build-tools DIR --platform DIR --manifest FILE
#          --package NAME --application-id ID --label LABEL
#          --min-sdk N --target-sdk N --version-code N --version-name NAME
#          [--java FILE]... [--library FILE]... [--assets DIR]

import argparse
import os
import subprocess
import tempfile
import zipfile

TIMESTAMP = (1980, 1, 1, 0, 0, 0)


def entry(archive, name, data, stored):
    info = zipfile.ZipInfo(name, TIMESTAMP)
    info.compress_type = zipfile.ZIP_STORED if stored else zipfile.ZIP_DEFLATED
    info.external_attr = 0o644 << 16
    archive.writestr(info, data)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("out")
    parser.add_argument("--jdk", required=True)
    parser.add_argument("--build-tools", required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--package", required=True)
    parser.add_argument("--application-id", required=True)
    parser.add_argument("--label", required=True)
    parser.add_argument("--min-sdk", required=True)
    parser.add_argument("--target-sdk", required=True)
    parser.add_argument("--version-code", required=True)
    parser.add_argument("--version-name", required=True)
    parser.add_argument("--java", action="append", default=[])
    parser.add_argument("--library", action="append", default=[])
    parser.add_argument("--assets")
    arguments = parser.parse_args()

    android_jar = os.path.join(arguments.platform, "android.jar")
    java = os.path.join(arguments.jdk, "bin", "java")
    scratch = tempfile.mkdtemp()

    with open(arguments.manifest) as file:
        manifest = file.read()
    manifest = manifest.replace("${be3Label}", arguments.label)
    manifest = manifest.replace("<manifest ", '<manifest package="{}" '.format(arguments.package), 1)
    manifest = manifest.replace("<application ", '<application android:extractNativeLibs="false" ', 1)
    manifest_path = os.path.join(scratch, "AndroidManifest.xml")
    with open(manifest_path, "w") as file:
        file.write(manifest)

    linked = os.path.join(scratch, "linked.apk")
    subprocess.run(
        [
            os.path.join(arguments.build_tools, "aapt2"),
            "link",
            "--manifest",
            manifest_path,
            "-I",
            android_jar,
            "--min-sdk-version",
            arguments.min_sdk,
            "--target-sdk-version",
            arguments.target_sdk,
            "--version-code",
            arguments.version_code,
            "--version-name",
            arguments.version_name,
            "--rename-manifest-package",
            arguments.application_id,
            "--debug-mode",
            "-o",
            linked,
        ],
        check=True,
    )

    dex = None
    if arguments.java:
        classes = os.path.join(scratch, "classes")
        subprocess.run(
            [
                os.path.join(arguments.jdk, "bin", "javac"),
                "-nowarn",
                "-Xlint:-options",
                "-source",
                "11",
                "-target",
                "11",
                "-classpath",
                android_jar,
                "-d",
                classes,
            ]
            + arguments.java,
            check=True,
        )
        compiled = sorted(
            os.path.join(directory, name)
            for directory, _, names in os.walk(classes)
            for name in names
            if name.endswith(".class")
        )
        dexed = os.path.join(scratch, "dex")
        os.makedirs(dexed)
        subprocess.run(
            [
                java,
                "-cp",
                os.path.join(arguments.build_tools, "lib", "d8.jar"),
                "com.android.tools.r8.D8",
                "--debug",
                "--min-api",
                arguments.min_sdk,
                "--lib",
                android_jar,
                "--output",
                dexed,
            ]
            + compiled,
            check=True,
        )
        dex = os.path.join(dexed, "classes.dex")

    unaligned = os.path.join(scratch, "unaligned.apk")
    with zipfile.ZipFile(unaligned, "w") as archive:
        with zipfile.ZipFile(linked) as source:
            for info in sorted(source.infolist(), key=lambda info: info.filename):
                entry(archive, info.filename, source.read(info), info.compress_type == zipfile.ZIP_STORED)
        if dex:
            with open(dex, "rb") as file:
                entry(archive, "classes.dex", file.read(), False)
        for library in sorted(arguments.library, key=os.path.basename):
            with open(library, "rb") as file:
                entry(archive, "lib/arm64-v8a/" + os.path.basename(library), file.read(), True)
        if arguments.assets:
            found = []
            for directory, _, names in os.walk(arguments.assets):
                for name in names:
                    path = os.path.join(directory, name)
                    found.append((os.path.relpath(path, arguments.assets).replace(os.sep, "/"), path))
            for name, path in sorted(found):
                with open(path, "rb") as file:
                    entry(archive, "assets/" + name, file.read(), False)

    subprocess.run(
        [os.path.join(arguments.build_tools, "zipalign"), "-P", "16", "-f", "4", unaligned, arguments.out],
        check=True,
    )


main()

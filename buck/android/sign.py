#!/usr/bin/env python3
#
# Signs an APK buck2 built, and with --install puts it on the device adb sees
# and starts it. This is the one step of the Android build that runs here
# rather than on a worker, because it needs the key, and the key is this
# machine's: Android refuses to install an update signed with a different key
# than the one already on the device, so every build has to be signed with the
# same one.
#
# The key is the keystore at --keystore, target/android-debug.keystore unless
# it says otherwise, with Android's debug passwords. One is made there if there
# is none yet. CI restores its own there from a secret, so that its builds stay
# installable over one another.
#
# Usage, through the runnables in crates/block-app/BUCK:
#   ./scripts/buck run //crates/block-app:android -- [--out APK] [--keystore FILE] [--install]

import argparse
import os
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(prog="./scripts/buck run //crates/block-app:android --")
    parser.add_argument("--jdk", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--build-tools", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--platform-tools", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--unsigned", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--out", default="target/android/block-app.apk", help="where the signed APK is written")
    parser.add_argument("--keystore", default="target/android-debug.keystore", help="the key it is signed with")
    parser.add_argument("--install", action="store_true", help="install it with adb and start it")
    arguments = parser.parse_args()

    java_home = arguments.jdk
    if not os.path.exists(arguments.keystore):
        print("Making a debug keystore at {}".format(arguments.keystore), flush=True)
        os.makedirs(os.path.dirname(arguments.keystore) or ".", exist_ok=True)
        subprocess.run(
            [
                os.path.join(java_home, "bin", "keytool"),
                "-genkeypair",
                "-keystore",
                arguments.keystore,
                "-storepass",
                "android",
                "-alias",
                "androiddebugkey",
                "-keypass",
                "android",
                "-dname",
                "CN=Android Debug,O=Android,C=US",
                "-keyalg",
                "RSA",
                "-keysize",
                "2048",
                "-validity",
                "10000",
            ],
            check=True,
            stdout=subprocess.DEVNULL,
        )

    os.makedirs(os.path.dirname(arguments.out) or ".", exist_ok=True)
    shutil.copyfile(arguments.unsigned, arguments.out)
    os.chmod(arguments.out, 0o644)
    environment = dict(os.environ, JAVA_HOME=java_home)
    subprocess.run(
        [
            os.path.join(java_home, "bin", "java"),
            "-jar",
            os.path.join(arguments.build_tools, "lib", "apksigner.jar"),
            "sign",
            "--ks",
            arguments.keystore,
            "--ks-pass",
            "pass:android",
            arguments.out,
        ],
        check=True,
        env=environment,
    )
    print("Wrote {}".format(arguments.out), flush=True)

    if not arguments.install:
        return
    package = subprocess.run(
        [os.path.join(arguments.build_tools, "aapt2"), "dump", "packagename", arguments.out],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.strip()
    adb = os.path.join(arguments.platform_tools, "adb")
    subprocess.run([adb, "install", "-r", arguments.out], check=True)
    subprocess.run([adb, "shell", "am", "start", "-n", "{}/com.be3.block.MainActivity".format(package)], check=True)


try:
    main()
except subprocess.CalledProcessError as error:
    sys.exit(error.returncode)

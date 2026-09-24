#!/usr/bin/env python3
#
# Lays out the app as it runs: the executable under cargo's name, --file beside
# it as it is (PDFium), and each plugin's manifest renamed <id>.plugin.json with
# the module it names and its .cwasm. For the web bundle, --tree copies in what
# wasm-bindgen wrote and --index writes plugins.json.
#
# Usage:
#   stage.py OUT [--executable=EXECUTABLE=NAME] [--file=FILE]... [--tree=DIR]...
#            [--index] (MANIFEST=MODULE [--artifact=CWASM])...

import json
import os
import shutil
import sys

out, *arguments = sys.argv[1:]

os.makedirs(out)
index = False
manifests = []

for argument in arguments:
    if argument.startswith("--executable="):
        executable, name = argument.removeprefix("--executable=").split("=", 1)
        shutil.copy2(executable, os.path.join(out, name))
        continue
    if argument.startswith("--file="):
        path = argument.removeprefix("--file=")
        shutil.copyfile(path, os.path.join(out, os.path.basename(path)))
        continue
    if argument.startswith("--tree="):
        tree = argument.removeprefix("--tree=")
        shutil.copytree(tree, out, dirs_exist_ok=True)
        continue
    if argument == "--index":
        index = True
        continue
    if argument.startswith("--artifact="):
        artifact = argument.removeprefix("--artifact=")
        shutil.copyfile(artifact, staged.removesuffix(".wasm") + ".cwasm")
        continue
    manifest, module = argument.split("=", 1)
    with open(manifest) as file:
        fields = json.load(file)
    if os.path.basename(module) != fields["entry_point"]:
        sys.exit("{} names {} as its entry point, but its module is {}".format(manifest, fields["entry_point"], module))
    shutil.copyfile(manifest, os.path.join(out, fields["id"] + ".plugin.json"))
    manifests.append(fields["id"] + ".plugin.json")
    staged = os.path.join(out, fields["entry_point"])
    shutil.copyfile(module, staged)

if index:
    with open(os.path.join(out, "plugins.json"), "w") as file:
        json.dump(manifests, file, indent=2)
        file.write("\n")

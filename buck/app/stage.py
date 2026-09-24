#!/usr/bin/env python3
#
# Lays out the app the way it runs: the executable, and beside it every
# plugin's manifest, renamed after the plugin's id, and the module the manifest
# names as its entry point. That is what the app's native plugin discovery
# scans the executable's own directory for. A module can be followed by the .cwasm compiled from it, which
# wasmtime maps in rather than compiling the module at launch, and which goes
# beside it.
#
# The web bundle is laid out the same way, with no executable: each --tree is
# a directory wasm-bindgen wrote, whose files go beside the page, and --index
# writes plugins.json, the list of the manifests staged, which is how the
# browser finds the plugins it cannot scan a directory for.
#
# Usage:
#   stage.py OUT [--executable=EXECUTABLE=NAME] [--file=FILE]... [--tree=DIR]...
#            [--index] (MANIFEST=MODULE [--artifact=CWASM])...
#
# The executable is renamed to the name cargo gives it, which buck2's does not
# share: buck2 names a binary after its crate. A --file goes beside it as it
# is: a library the app loads at run time, such as PDFium. With no executable
# the directory is the plugins alone, which is what CI ships beside every
# platform's app.

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

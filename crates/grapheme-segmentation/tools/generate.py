from pathlib import Path
import tempfile
from urllib.request import urlopen

VERSION = "17.0.0"
ROOT = Path(__file__).resolve().parents[1]
BASE = f"https://www.unicode.org/Public/{VERSION}/ucd/"
FILES = {
    "GraphemeBreakProperty.txt": "auxiliary/GraphemeBreakProperty.txt",
    "DerivedCoreProperties.txt": "DerivedCoreProperties.txt",
    "emoji-data.txt": "emoji/emoji-data.txt",
    "GraphemeBreakTest.txt": "auxiliary/GraphemeBreakTest.txt",
}


def entries(file, pick):
    result = []
    for line in file.read_text().splitlines():
        fields = [field.strip() for field in line.split("#")[0].split(";")]
        if len(fields) < 2:
            continue
        value = pick(fields)
        if value is None:
            continue
        bounds = fields[0].split("..")
        result.append((int(bounds[0], 16), int(bounds[-1], 16), value))
    return sorted(result)


def main():
    with tempfile.TemporaryDirectory() as directory:
        data = Path(directory)
        for name, relative in FILES.items():
            (data / name).write_bytes(urlopen(BASE + relative).read())
        gcb = entries(data / "GraphemeBreakProperty.txt", lambda f: f[1].replace("_", "").replace("LVT", "Lvt").replace("ZWJ", "Zwj"))
        incb = entries(data / "DerivedCoreProperties.txt", lambda f: f[2] if f[1] == "InCB" else None)
        ep = entries(data / "emoji-data.txt", lambda f: "true" if f[1] == "Extended_Pictographic" else None)
        output = ["use super::{Gcb, Incb};\n"]
        for name, rows, typ in [("GCB", gcb, "Gcb"), ("INCB", incb, "Incb"), ("EXTENDED_PICTOGRAPHIC", ep, "bool")]:
            output.append(f"pub(super) static {name}: &[(u32, u32, {typ})] = &[\n")
            for start, end, value in rows:
                value = value if typ == "bool" else f"{typ}::{value}"
                output.append(f"    (0x{start:x}, 0x{end:x}, {value}),\n")
            output.append("];\n")
        (ROOT / "src/tables.rs").write_text("".join(output))
        fixture = (data / "GraphemeBreakTest.txt").read_text()
        (ROOT / "src/tests/GraphemeBreakTest.txt").write_text(
            "".join(line.rstrip() + "\n" for line in fixture.splitlines())
        )


if __name__ == "__main__":
    main()

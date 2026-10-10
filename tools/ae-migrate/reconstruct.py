"""Recover an EffectCraft AE observation from an XML copy saved by export.jsx."""

import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

PREFIX = "EC_EXPORT_V1_"
NAMESPACE = "{http://www.adobe.com/products/aftereffects}"
MAX_XML_BYTES = 512 * 1024 * 1024
MAX_JSON_CHARS = 256 * 1024 * 1024


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: python tools/ae-migrate/reconstruct.py INPUT.aepx OUTPUT.json", file=sys.stderr)
        return 2
    source, output = map(Path, sys.argv[1:])
    if source.stat().st_size > MAX_XML_BYTES:
        raise ValueError("XML copy is too large")

    parts = {}
    for _, node in ET.iterparse(source, events=("end",)):
        if node.tag == NAMESPACE + "Item":
            name = node.findtext(NAMESPACE + "string")
            if name and name.startswith(PREFIX):
                suffix = name[len(PREFIX):]
                if suffix in parts:
                    raise ValueError(f"duplicate export chunk: {suffix}")
                parts[suffix] = node.findtext(NAMESPACE + "Comment") or ""
            node.clear()

    if "ERROR" in parts:
        raise ValueError(parts["ERROR"])
    meta = json.loads(parts["META"])
    count = meta["chunks"]
    length = meta["length"]
    if not isinstance(count, int) or count < 0 or count > 100_000:
        raise ValueError("invalid export chunk count")
    if not isinstance(length, int) or length < 0 or length > MAX_JSON_CHARS:
        raise ValueError("invalid export size")
    chunks = [parts[f"{number:05d}"] for number in range(count)]
    joined = "".join(chunks)
    if len(joined) != length or parts.get("DONE") != str(count):
        raise ValueError("incomplete export")
    data = json.loads(joined)
    if data.get("format") != "effectcraft-ae-observation-v1":
        raise ValueError("unsupported export format")
    if len(data["items"]) != meta["items"]:
        raise ValueError("item count mismatch")
    output.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
    print(f"recovered {meta['items']} items to {output}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ET.ParseError, ValueError, KeyError, TypeError) as error:
        print(f"reconstruct: {error}", file=sys.stderr)
        sys.exit(1)

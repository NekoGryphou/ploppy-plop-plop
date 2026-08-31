#!/usr/bin/env python3
"""Generate the Decky backend's compact protocol descriptors from proto3."""

from __future__ import annotations

import argparse
import hashlib
import re
from pathlib import Path


PROJECT = Path(__file__).resolve().parents[2]
SCHEMA = PROJECT / "proto" / "decky_power.proto"
OUTPUT = PROJECT / "decky" / "py_modules" / "decky_power" / "generated_schema.py"


def generate(source: str) -> str:
    cleaned = re.sub(r"//.*", "", source)
    enums = {
        name
        for name, _ in re.findall(r"enum\s+(\w+)\s*\{([^}]*)\}", cleaned, re.DOTALL)
    }
    messages: dict[str, list[tuple[str, int, str]]] = {}
    field_pattern = re.compile(
        r"\b(string|bytes|uint32|bool|\w+)\s+(\w+)\s*=\s*(\d+)\s*;"
    )
    for name, body in re.findall(
        r"message\s+(\w+)\s*\{([^}]*)\}", cleaned, re.DOTALL
    ):
        fields: list[tuple[str, int, str]] = []
        for field_type, field_name, number in field_pattern.findall(body):
            wire_type = "enum" if field_type in enums else field_type
            if wire_type not in {"string", "bytes", "uint32", "bool", "enum"}:
                raise ValueError(f"unsupported proto field type: {field_type}")
            fields.append((field_name, int(number), wire_type))
        messages[name] = fields
    if not messages:
        raise ValueError("no Protobuf messages found")

    digest = hashlib.sha256(source.encode()).hexdigest()
    lines = [
        '"""Generated from proto/decky_power.proto. Do not edit by hand."""',
        "",
        f'SCHEMA_SHA256 = "{digest}"',
        "MESSAGES: dict[str, dict[str, tuple[int, str]]] = {",
    ]
    for message_name, fields in messages.items():
        lines.append(f'    "{message_name}": {{')
        for field_name, number, field_type in fields:
            lines.append(
                f'        "{field_name}": ({number}, "{field_type}"),'
            )
        lines.append("    },")
    lines.extend(["}", ""])
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check", action="store_true", help="fail if the generated file is stale"
    )
    arguments = parser.parse_args()
    generated = generate(SCHEMA.read_text("utf-8"))
    if arguments.check:
        if not OUTPUT.exists() or OUTPUT.read_text("utf-8") != generated:
            print(
                "Decky Protobuf descriptors are stale; run "
                "./scripts/decky/generate-python-proto.py",
            )
            return 1
        return 0
    OUTPUT.write_text(generated, "utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Validate the portable Windows artifact without extra Python packages."""
import struct
import sys
from pathlib import Path


def check(path: Path) -> None:
    data = path.read_bytes()
    assert data[:2] == b"MZ", "Not a Windows executable"
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    assert data[pe:pe + 4] == b"PE\0\0", "Invalid PE signature"
    coff = pe + 4
    machine, sections = struct.unpack_from("<HH", data, coff)
    assert machine == 0x8664, "Expected x64"
    optional_size = struct.unpack_from("<H", data, coff + 16)[0]
    optional = coff + 20
    assert struct.unpack_from("<H", data, optional)[0] == 0x20B, "Expected PE32+"
    assert struct.unpack_from("<H", data, optional + 68)[0] == 2, "Console subsystem is not portable GUI packaging"
    section_table = optional + optional_size

    def offset(rva: int) -> int:
        for i in range(sections):
            entry = section_table + i * 40
            virtual_size, address, raw_size, pointer = struct.unpack_from("<IIII", data, entry + 8)
            if address <= rva < address + max(virtual_size, raw_size):
                return pointer + rva - address
        raise AssertionError(f"Unmapped RVA: {rva:x}")

    imports_rva = struct.unpack_from("<I", data, optional + 120)[0]
    cursor = offset(imports_rva)
    imports = []
    while any(data[cursor:cursor + 20]):
        name_rva = struct.unpack_from("<I", data, cursor + 12)[0]
        start = offset(name_rva)
        imports.append(data[start:data.index(b"\0", start)].decode("ascii"))
        cursor += 20
    assert not any(name.lower().startswith(("vcruntime", "msvcp", "ucrtbase")) for name in imports), imports
    assert b"PerMonitorV2" in data, "Missing DPI manifest"
    assert b"GLYPH" in data, "Missing application identity"
    print(f"OK: {path} | x64 Windows GUI | {len(data) / 1024 / 1024:.2f} MiB | static MSVC CRT")
    print("System DLL imports: " + ", ".join(imports))


if __name__ == "__main__":
    check(Path(sys.argv[1] if len(sys.argv) > 1 else "GLYPH.exe"))

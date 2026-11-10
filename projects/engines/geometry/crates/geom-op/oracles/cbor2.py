"""The cbor2 oracle for geom-op's canonical CBOR encoder (piece G86; chapter 28.10's
out-of-process oracles).

Reads cases on standard input, one per line: the kernel's encoding in hexadecimal, a tab, and the
value in the JSON form of geom-op's tests/common (one key per value naming its kind: "u" and "i"
integers as decimal strings, "f" a double as the sixteen hexadecimal digits of its bits, "b" bytes
in hexadecimal, "t" text, "a" an array, "m" a map as [key, value] pairs, "ci" and "cf" the typed
arrays of RFC 8746, tags 79 and 86, "set" and "sorted" arrays sorted by their elements' encodings).
Writes one line per case: cbor2's canonical encoding of the value, in hexadecimal, a tab, and
"same" if cbor2 decodes the kernel's bytes back to the value, else "differs".

cbor2 (MIT, Alex Grönholm) is run, never linked: its canonical mode sorts map keys by length first
(RFC 8949 section 4.2.3), which agrees with section 4.2.1's bytewise order for unsigned keys, the
only map keys SKG writes (24.3). A set or sorted list is built here from cbor2's own encodings of
its elements, sorted by their bytes (32.19), so cbor2 decides every byte.
"""

import json
import struct
import sys

import cbor2


def build(j):
    """The Python value a JSON form describes, as cbor2 encodes it."""
    ((kind, x),) = j.items()
    if kind in ("u", "i"):
        return int(x)
    if kind == "f":
        return struct.unpack(">d", bytes.fromhex(x))[0]
    if kind == "b":
        return bytes.fromhex(x)
    if kind == "t":
        return x
    if kind == "a":
        return [build(v) for v in x]
    if kind == "m":
        return {int(k): build(v) for k, v in x}
    if kind == "ci":
        return cbor2.CBORTag(79, b"".join(struct.pack("<q", int(v)) for v in x))
    if kind == "cf":
        return cbor2.CBORTag(86, b"".join(bytes.fromhex(v)[::-1] for v in x))
    if kind in ("set", "sorted"):
        encodings = sorted(cbor2.dumps(build(v), canonical=True) for v in x)
        if kind == "set" and len(set(encodings)) != len(encodings):
            raise ValueError("a set with a repeat")
        return [cbor2.loads(e) for e in encodings]
    raise ValueError("no such kind: " + kind)


def same(a, b):
    """Equal values, doubles compared by their bits, so -0.0 is not 0.0."""
    if isinstance(a, float) or isinstance(b, float):
        return (
            isinstance(a, float)
            and isinstance(b, float)
            and struct.pack(">d", a) == struct.pack(">d", b)
        )
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    if isinstance(a, dict) and isinstance(b, dict):
        return list(a) == list(b) and all(same(a[k], b[k]) for k in a)
    if isinstance(a, cbor2.CBORTag) and isinstance(b, cbor2.CBORTag):
        return a.tag == b.tag and same(a.value, b.value)
    return type(a) is type(b) and a == b


def main():
    for line in sys.stdin:
        ours, value = line.rstrip("\n").split("\t", 1)
        expected = build(json.loads(value))
        theirs = cbor2.dumps(expected, canonical=True).hex()
        decoded = cbor2.loads(bytes.fromhex(ours))
        print(theirs + "\t" + ("same" if same(decoded, expected) else "differs"))


if __name__ == "__main__":
    main()

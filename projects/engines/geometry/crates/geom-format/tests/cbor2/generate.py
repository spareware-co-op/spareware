"""Writes the cbor2 cross-check vectors for geom-format's strict CBOR decoder and text form.

cbor2 (https://github.com/agronholm/cbor2, MIT) is an independent CBOR implementation in Python.
For every input this script asks cbor2 whether the bytes are SKG's deterministic CBOR (24.3):

1. cbor2 decodes the bytes leniently, and must consume all of them;
2. the value must lie in SKG's subset (integers, byte and text strings, arrays, maps, finite floats,
   false, true, null, and tags 79 and 86 over whole columns, finite for 86);
3. the value's canonical encoding must be exactly the input. That encoding is cbor2's own with
   `canonical=True`, except that map pairs are put in the bytewise order of their keys' encodings
   (RFC 8949 section 4.2.1): cbor2 sorts keys length first, RFC 7049's older rule.

For an accepted input the script also writes the text form, by this file's own printer of the
layout geom-format's `cbor::text` documents, from cbor2's decoded value. The Rust test
(`tests/cbor2.rs`) checks that the decoder gives the same verdict on every input and, for accepted
ones, the same text.

Run from this folder with cbor2 6.1.5 importable:

    python3 generate.py > vectors.jsonl
    python3 generate.py --short > short.txt

The seed is fixed, so the files are a pure function of this script and cbor2's version.
"""

import io
import json
import math
import random
import struct
import sys

import cbor2

SEED = 0x5350415245_4B4954
DEPTH = 128


def head(major, n):
    """The shortest head, through cbor2's own encoder."""
    out = io.BytesIO()
    cbor2.CBOREncoder(out).encode_length(major, n)
    return out.getvalue()


def canon(v):
    """The canonical encoding of a decoded value, or None if it lies outside SKG's subset."""
    if isinstance(v, bool) or v is None:
        return cbor2.dumps(v, canonical=True)
    if isinstance(v, int):
        return cbor2.dumps(v, canonical=True)
    if isinstance(v, float):
        if not math.isfinite(v):
            return None
        return cbor2.dumps(v, canonical=True)
    if isinstance(v, (bytes, str)):
        return cbor2.dumps(v, canonical=True)
    if isinstance(v, (list, tuple)):
        parts = [canon(x) for x in v]
        if any(p is None for p in parts):
            return None
        return head(4, len(v)) + b"".join(parts)
    if isinstance(v, dict):
        pairs = []
        for k, x in v.items():
            ck, cx = canon(k), canon(x)
            if ck is None or cx is None:
                return None
            pairs.append((ck, cx))
        pairs.sort(key=lambda p: p[0])
        return head(5, len(pairs)) + b"".join(k + x for k, x in pairs)
    if isinstance(v, cbor2.CBORTag):
        if v.tag not in (79, 86) or not isinstance(v.value, bytes) or len(v.value) % 8:
            return None
        if v.tag == 86:
            for i in range(0, len(v.value), 8):
                if not math.isfinite(struct.unpack("<d", v.value[i : i + 8])[0]):
                    return None
        return head(6, v.tag) + cbor2.dumps(v.value, canonical=True)
    return None


def depth(v):
    if isinstance(v, (list, tuple)):
        return 1 + max((depth(x) for x in v), default=0)
    if isinstance(v, dict):
        return 1 + max((max(depth(k), depth(x)) for k, x in v.items()), default=0)
    if isinstance(v, cbor2.CBORTag):
        return 1
    if isinstance(v, int) and not -(2**64) <= v < 2**64:
        return 1
    return 0


class Ambiguous(Exception):
    """Python's equality joins map keys CBOR keeps apart (0, 0.0, -0.0 and false are one key to a
    dict), so cbor2 cannot judge a map holding two of them; such inputs are left out. Repeated keys
    are refused by the Rust attack tests instead."""


def verdict(b):
    """`(True, value)` if cbor2 says `b` is SKG's deterministic CBOR, else `(False, None)`; the
    value may itself be None, CBOR's null."""
    stream = io.BytesIO(b)
    try:
        v = cbor2.CBORDecoder(stream, allow_duplicate_keys=False, max_depth=4 * DEPTH).decode()
    except cbor2.CBORDecodeError as e:
        if "Duplicate map key" in str(e):
            raise Ambiguous from e
        return False, None
    except Exception:
        return False, None
    if stream.tell() != len(b) or depth(v) > DEPTH:
        return False, None
    return (True, v) if canon(v) == b else (False, None)


def hex_float(x):
    sign = "-" if math.copysign(1.0, x) < 0 else ""
    if x == 0:
        return sign + "0x0p+0"
    m, e = math.frexp(abs(x))  # abs(x) = m * 2**e, 0.5 <= m < 1, exact
    fraction = int(m * 2**53) - 2**52  # the 52 bits after the leading 1 of 1.f * 2**(e-1)
    digits = format(fraction, "013x").rstrip("0")
    exponent = e - 1
    return f"{sign}0x1{'.' + digits if digits else ''}p{'+' if exponent >= 0 else '-'}{abs(exponent)}"


def quote(t):
    out = ['"']
    for c in t:
        o = ord(c)
        if c == '"':
            out.append('\\"')
        elif c == "\\":
            out.append("\\\\")
        elif c == "\n":
            out.append("\\n")
        elif c == "\r":
            out.append("\\r")
        elif c == "\t":
            out.append("\\t")
        elif c == "\b":
            out.append("\\b")
        elif c == "\f":
            out.append("\\f")
        elif o < 0x20 or o == 0x7F:
            out.append(f"\\u{o:04x}")
        else:
            out.append(c)
    out.append('"')
    return "".join(out)


def text(v, level=0):
    """The text form of a decoded value in geom-format's layout."""
    if v is False:
        return "false"
    if v is True:
        return "true"
    if v is None:
        return "null"
    if isinstance(v, int):
        if v >= 2**64:
            return f"2(h'{v.to_bytes((v.bit_length() + 7) // 8, 'big').hex()}')"
        if v < -(2**64):
            n = -1 - v
            return f"3(h'{n.to_bytes((n.bit_length() + 7) // 8, 'big').hex()}')"
        return str(v)
    if isinstance(v, float):
        return hex_float(v)
    if isinstance(v, bytes):
        return f"h'{v.hex()}'"
    if isinstance(v, str):
        return quote(v)
    if isinstance(v, cbor2.CBORTag):
        return f"{v.tag}(h'{v.value.hex()}')"
    pad = "  " * (level + 1)
    if isinstance(v, (list, tuple)):
        if not v:
            return "[]"
        items = [pad + text(x, level + 1) for x in v]
        return "[\n" + ",\n".join(items) + "\n" + "  " * level + "]"
    if isinstance(v, dict):
        if not v:
            return "{}"
        pairs = sorted(v.items(), key=lambda p: canon(p[0]))
        items = [pad + text(k, level + 1) + ": " + text(x, level + 1) for k, x in pairs]
        return "{\n" + ",\n".join(items) + "\n" + "  " * level + "}"
    raise ValueError(v)


def random_float(rng):
    while True:
        kind = rng.randrange(5)
        if kind == 0:
            x = struct.unpack("<e", rng.randbytes(2))[0]
        elif kind == 1:
            x = struct.unpack("<f", rng.randbytes(4))[0]
        elif kind == 2:
            x = struct.unpack("<d", rng.randbytes(8))[0]
        elif kind == 3:
            x = rng.choice([0.0, -0.0, 5e-324, 2.2250738585072014e-308, 1.7976931348623157e308, 65504.0, 65520.0])
        else:
            x = rng.randrange(-1000, 1000) / 8
        if math.isfinite(x):
            return x


def random_value(rng, level):
    leaf = level == 0 or rng.randrange(3) == 0
    kind = rng.randrange(9) if leaf else 9 + rng.randrange(2)
    if kind == 0:
        return rng.choice([0, 23, 24, 255, 256, 65535, 65536, 2**32 - 1, 2**32, 2**64 - 1, rng.getrandbits(rng.randrange(1, 65))])
    if kind == 1:
        return -1 - rng.getrandbits(rng.randrange(1, 65))
    if kind == 2:
        return rng.choice([1, -1]) * (2**64 + rng.getrandbits(rng.randrange(1, 100)))
    if kind == 3:
        return rng.randbytes(rng.randrange(0, 30))
    if kind == 4:
        return "".join(rng.choice('aZ0 "\\\n\x00\x1f\x7fé 𝄞/') for _ in range(rng.randrange(0, 8)))
    if kind == 5:
        return random_float(rng)
    if kind == 6:
        return rng.choice([True, False, None])
    if kind == 7:
        return cbor2.CBORTag(86, b"".join(struct.pack("<d", random_float(rng)) for _ in range(rng.randrange(0, 4))))
    if kind == 8:
        return cbor2.CBORTag(79, rng.randbytes(8 * rng.randrange(0, 4)))
    if kind == 9:
        return [random_value(rng, level - 1) for _ in range(rng.randrange(0, 4))]
    keys = {}
    for _ in range(rng.randrange(0, 4)):
        k = random_value(rng, 0)
        if isinstance(k, (bytes, str, int, float, bool)) or k is None:
            keys[k] = random_value(rng, level - 1)
    return keys


def mutate(rng, b):
    b = bytearray(b)
    for _ in range(rng.randrange(1, 4)):
        op = rng.randrange(4)
        if op == 0 and b:
            b[rng.randrange(len(b))] ^= 1 << rng.randrange(8)
        elif op == 1 and b:
            b[rng.randrange(len(b))] = rng.randrange(256)
        elif op == 2:
            b.insert(rng.randrange(len(b) + 1), rng.randrange(256))
        elif b:
            del b[rng.randrange(len(b))]
    return bytes(b)


def lenient(rng, v):
    """Another encoding of v: cbor2's default, which writes every float as a double and maps in
    insertion order, or the length-first canonical order."""
    try:
        return cbor2.dumps(v, canonical=rng.random() < 0.5)
    except Exception:
        return None


def main():
    rng = random.Random(SEED)
    if "--short" in sys.argv:
        inputs = [bytes([n]) for n in range(256)] + [n.to_bytes(2, "big") for n in range(65536)]
        for b in inputs:
            try:
                print(f"{b.hex()} {'ok' if verdict(b)[0] else 'refused'}")
            except Ambiguous:
                pass
        return
    seen = set()

    def emit(b):
        if b is None or b in seen or len(b) > 4096:
            return
        seen.add(b)
        try:
            ok, v = verdict(b)
        except Ambiguous:
            return
        record = {"hex": b.hex(), "ok": ok}
        if ok:
            record["text"] = text(v)
        print(json.dumps(record, ensure_ascii=False))

    for _ in range(700):
        v = random_value(rng, 3)
        b = canon(v)
        emit(b)
        emit(lenient(rng, v))
        for _ in range(3):
            emit(mutate(rng, b))


if __name__ == "__main__":
    main()

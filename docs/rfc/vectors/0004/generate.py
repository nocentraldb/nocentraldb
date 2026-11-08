"""Writes the byte-exact examples E1 to E11 of RFC 0004 Amendment 1
(docs/rfc/0004-merge-rules-basic-types.md, "Byte-exact examples") in this
directory, from their descriptions in the RFC and independently of the Rust
core and of the tooling tests' reader: with cbor2 (6.2.1, canonical mode).
The gates do not run this script; scripts/rfc-0004-examples.test.mjs checks
that the RFC's hex equals the committed files.

Run: python3 -I generate.py  (needs cbor2==6.2.1)

cbor2's canonical mode sorts map keys length first, RFC 0002 bytewise. The
two agree for the maps below (unsigned keys only); do not mix unsigned and
text keys in one map here.
"""

import pathlib

import cbor2

HERE = pathlib.Path(__file__).parent

# The fixed ids of the RFC's examples.
T1 = bytes([0x11] * 16)  # the object every example writes to
T2 = bytes([0x22] * 16)  # the link target
A = bytes([0xAA] * 32)  # change id A
B = bytes([0xBB] * 32)  # change id B

# Operation numbers (Amendment 1, "Operation numbers").
SET, ADD, REMOVE = 1, 2, 3


def enc(value):
    return cbor2.dumps(value, canonical=True)


def ids(*operation_ids):
    """A list of operation ids: sorted by their canonical bytes, no repeats."""
    unique = {enc(list(i)): list(i) for i in operation_ids}
    return [unique[key] for key in sorted(unique)]


def operation(field, number, arguments):
    return {1: T1, 2: "tasks", 3: field, 4: number, 5: arguments}


def set_(field, value, replaces=()):
    return operation(field, SET, {1: value, 2: ids(*replaces)})


def add(field, element, replaces=()):
    return operation(field, ADD, {1: element, 2: ids(*replaces)})


def remove(field, element, observed):
    return operation(field, REMOVE, {1: element, 3: ids(*observed)})


EXAMPLES = {
    # E1. set, the first write to a string.
    "E1": set_("title", "Ship v1"),
    # E2. set, an integer that replaces a winner and a conflict.
    "E2": set_("priority", 3, replaces=[(B, 1), (A, 0)]),
    # E3. set, clearing a field.
    "E3": set_("title", None, replaces=[(A, 0)]),
    # E4. add, a fresh addition to a set.
    "E4": add("tags", "urgent"),
    # E5. remove, from a set.
    "E5": remove("tags", "urgent", observed=[(A, 2)]),
    # E6. add, a deliberate re-add after the lost removal [B, 0].
    "E6": add("tags", "urgent", replaces=[(B, 0)]),
    # E7. set, a link to one target.
    "E7": set_("parent", T2),
    # E8. add, a link to one more target.
    "E8": add("dependsOn", T2),
    # E9. remove, a link target.
    "E9": remove("dependsOn", T2, observed=[(A, 1)]),
    # E10. E1 with an operation number the type does not define.
    "E10": operation("title", 9, {1: "Ship v1", 2: []}),
    # E11. E1 with the "may ignore" argument 64.
    "E11": operation("title", SET, {1: "Ship v1", 2: [], 64: "hint"}),
}


def write(name, data):
    text = data.hex()
    lines = [text[i : i + 64] for i in range(0, len(text), 64)]
    (HERE / f"{name}.hex").write_text("\n".join(lines) + "\n")


for name, value in EXAMPLES.items():
    data = enc(value)
    assert enc(cbor2.loads(data)) == data, name
    write(name, data)

# Element types and instances, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record it uses unchanged and the [parameter values](../construct-params/FORMAT.md) it stores. **We may stop writing an old version. We never stop reading it.** [`src/element_type.rs`](src/element_type.rs), [`src/instance.rs`](src/instance.rs) and [`src/record.rs`](src/record.rs) are the reader library.

## A type

A type is an element record like any other (the records in the database are the truth), stored once. Its instances name it and never hold a copy of its values, so changing a type changes what every instance of it reads.

| Field | On a type record |
| --- | --- |
| `id` | The type's permanent ID |
| `document` | The document it belongs to |
| `kind` | `type` |
| `name` | The name people see: trimmed, not empty, unique within its category without regard to case |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]`: a type has no place and no shape |
| `recipe` | `{"owner":"construct","schema":1,"op":"type","op_version":1,"args":{}}`: builds nothing; its `op_version` is this format's version |
| `props` | The entries below |

| `props` key | Value |
| --- | --- |
| `@category` | The type's category ID ([construct-categories](../construct-categories/FORMAT.md)) in decimal, no leading zero |
| `@params` | The permanent IDs of every parameter its elements have, held by the type or by each instance, strictly ascending, joined by `,`; empty for none |
| a parameter ID | The type's value of a parameter the type holds and `@params` lists, in its stored text. A parameter it lists with no entry reads its default |

```json
{"id":"T-820","document":"DOC-1","kind":"type","name":"Single 820","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"type","op_version":1,"args":{}},"props":{"@category":"300","@params":"P-finish,P-height,P-mark,P-sill,P-width","P-width":"length:820"},"references":[]}
```

A key starting with `@` is reserved here. A permanent ID always starts with a letter or digit, so a reserved key is never a parameter's ID.

## An instance

An instance is the element record of a placed element: its own kind, placement and recipe. In its `props` this format owns:

| `props` key | Value |
| --- | --- |
| `@type` | The permanent ID of its type |
| a parameter ID | Its own value of a parameter each instance holds and its type's `@params` lists |

`{"@type":"T-820","P-mark":"text:D01","P-sill":"length:150"}`

Its category is its type's. Other keys in its `props` are not this format's and are read as they are.

## Reading a value

An instance reads a parameter from, in order: its own value, its type's value, the parameter's default, else nothing. A parameter the type holds is never on an instance, and one each instance holds is never on the type.

## Rules a reader checks

A record that breaks one is refused, never repaired: a type record's fixed fields are as above; `@category` and `@params` are present and spelt as above; every listed parameter and the category are known; no other reserved key; every other key is a listed parameter the type holds, with a value in its one spelling. An instance names a known type, holds no value its type holds or lacks, and has no other reserved key. The same data always gives the same bytes.

## Later versions

A family type (a type made from a family recipe) will be a type record whose `recipe` names that recipe and its type row; a system type's layers (walls) will be type data on its record. Each is a new version here, read beside version 1 for ever.

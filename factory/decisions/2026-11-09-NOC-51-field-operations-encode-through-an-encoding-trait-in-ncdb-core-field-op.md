# 2026-11-09 · NOC-51 · Field operations encode through an `Encoding` trait in `ncdb_core::field_op`

- Decided by: coding agent (NOC-51)
- Why: RFC 0004 Amendment 1 reads an operation number in the context of the field's type, so each field type (`BasicValue`, `Link`, `Set`, `Links`) implements `Encoding: FieldType`, which gives the operation number and arguments of its operation and decodes an RFC 0002 operation map into it, or returns the `OperationParkReason` it parks with (`UnknownOperation`, `UnknownArgument`, `MalformedArguments`, added beside RFC 0005's reasons in `log.rs`). The operation's id, author and stamp come from its change (`Origin`), as the amendment says. Calling the decoder from `receive`'s fold needs the field's type from the model, which is NOC-39's fold; this ticket does not wire it in.
- Instead of: methods on `FieldType` itself, which every field type would need, also those whose operations no RFC encodes yet; or one free decoder keyed by type name, which `model.lock.json` does not reach yet.
- Topics: field-types, rfc-0004, encoding

// The names an editor declares to a rich-text binding beyond the field's
// vocabulary, its placeholders (RFC 0013, "An editor that cannot show a
// node"; NOC-461). A module of its own, so the SDK's entry exports it on a
// line of its own.

/** Names an editor knows besides the field's vocabulary: nodes, marks, and attributes by node. */
export interface KnownNames {
  readonly nodes: readonly string[];
  readonly marks: readonly string[];
  readonly attributes?: Readonly<Record<string, readonly string[]>>;
}

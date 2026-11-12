// What `nc.add` and `nc.remove` give: elements to add to, or remove from,
// a set or many links (RFC 0009, "Field operations per field type").
// Internal: the app sees only the type `Elements`.

export class SetElements {
  constructor(
    readonly add: boolean,
    readonly elements: unknown[],
  ) {}
}

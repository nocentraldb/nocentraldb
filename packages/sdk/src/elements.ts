// What `nc.add` and `nc.remove` give: elements to add to, or remove from,
// a set or many links (RFC 0009, "Field operations per field type").
// Internal: the app sees only the type `Elements`. Plain fields, not
// parameter properties: `ncdb model build` runs the SDK's source under
// Node's type stripping, which supports only erasable syntax.

export class SetElements {
  readonly add: boolean;
  readonly elements: unknown[];

  constructor(add: boolean, elements: unknown[]) {
    this.add = add;
    this.elements = elements;
  }
}

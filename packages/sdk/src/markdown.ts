// What `nc.richText.fromMarkdown` gives: a first document from Markdown,
// which a create or an update writes into a rich-text field (RFC 0013,
// "The first content"; NOC-578). Internal: the app sees only the type
// `FirstDocument`. A plain field, not a parameter property: `ncdb model
// build` runs the SDK's source under Node's type stripping, which supports
// only erasable syntax.

/** A first document from Markdown, which only a rich-text field takes. */
export interface FirstDocument {
  readonly "nc.firstDocument": true;
}

export class Markdown {
  readonly markdown: string;

  constructor(markdown: string) {
    this.markdown = markdown;
  }
}

/** The first document `markdown` writes. */
export const fromMarkdown = (markdown: string): FirstDocument =>
  new Markdown(markdown) as unknown as FirstDocument;

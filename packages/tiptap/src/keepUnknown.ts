// NcKeepUnknown, for an app that builds its own TipTap editor (RFC 0013,
// "An editor that cannot show a node"): it widens the editor's schema from
// the field's document (measure 1) and refuses, as the editor is created,
// an editor without `enableContentCheck` (measure 2), with
// `contentCheckMissing` (Carsten, 2026-11-22, NOC-467; NOC-538, Q3).
// `ncEditor` sets both itself.

import { nc } from "@nocentraldb/sdk";
import { type AnyExtension, Extension } from "@tiptap/core";
import type * as Y from "yjs";
import { widen } from "./widen.ts";

export interface NcKeepUnknownOptions {
  /** The field's document, `body.doc`. */
  document: Y.Doc | null;
  /** The editor's other extensions, against whose schema the document is widened. */
  extensions: AnyExtension[];
}

export const NcKeepUnknown = Extension.create<NcKeepUnknownOptions>({
  name: "ncKeepUnknown",
  addOptions: () => ({ document: null, extensions: [] }),
  addExtensions() {
    const { document, extensions } = this.options;
    // Without the editor's other extensions there is no schema to widen
    // against; the content check still holds.
    return document && extensions.length > 0 ? widen(document, extensions) : [];
  },
  onBeforeCreate() {
    if (!this.editor.options.enableContentCheck) {
      throw new nc.Error(
        "contentCheckMissing",
        "the editor of a rich-text field needs enableContentCheck: true, or it deletes what its schema does not know",
      );
    }
  },
});

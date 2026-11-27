// NcMention (RFC 0013, "Mentions"; NOC-461): an inline node named
// `mention` that holds a reference to an object, the target's `space` and
// `id` as RFC 0009 writes them, and never a name. The editor shows the
// name the app's `label` gives for the target when it renders; until then,
// and when `label` gives nothing, as after an erasure, it shows a neutral
// placeholder. The name never reaches the document, the HTML or the JSON.
//
// The options' shape is option (a) and waits for the owner (NOC-579).
import { mergeAttributes, Node } from "@tiptap/core";

/** What a mention refers to: the target object's space id and object id. */
export interface NcMentionTarget {
  readonly space: string;
  readonly id: string;
}

export interface NcMentionOptions {
  /** The name to show for the target; nothing shows the placeholder. */
  label: (
    target: NcMentionTarget,
  ) => string | undefined | Promise<string | undefined>;
}

/** What the editor shows before it has a name, and when there is none. */
const PLACEHOLDER = "@…";

const shown = (name: string | undefined) =>
  typeof name === "string" && name.length > 0 ? `@${name}` : PLACEHOLDER;

/** The reference node: `space` and `id`, and no name. */
export const NcMention = Node.create<NcMentionOptions>({
  name: "mention",
  group: "inline",
  inline: true,
  atom: true,
  selectable: false,

  addOptions: () => ({ label: () => undefined }),

  addAttributes: () => ({
    space: {
      default: null,
      parseHTML: (element) => element.getAttribute("data-space"),
      renderHTML: (attributes) => ({ "data-space": attributes.space }),
    },
    id: {
      default: null,
      parseHTML: (element) => element.getAttribute("data-id"),
      renderHTML: (attributes) => ({ "data-id": attributes.id }),
    },
  }),

  parseHTML: () => [{ tag: 'span[data-type="mention"]' }],

  // The HTML holds the reference and a bare "@": the name is only shown.
  renderHTML: ({ HTMLAttributes }) => [
    "span",
    mergeAttributes({ "data-type": "mention" }, HTMLAttributes),
    "@",
  ],

  renderText: () => "@",

  addNodeView() {
    const { label } = this.options;
    return ({ node }) => {
      const dom = document.createElement("span");
      dom.setAttribute("data-type", "mention");
      dom.contentEditable = "false";
      // A name asked for earlier attributes is not shown for later ones.
      let asked = 0;
      const show = (space: string, id: string) => {
        dom.setAttribute("data-space", space);
        dom.setAttribute("data-id", id);
        const now = ++asked;
        let name: ReturnType<NcMentionOptions["label"]>;
        try {
          name = label({ space, id });
        } catch {
          name = undefined;
        }
        if (name instanceof Promise) {
          dom.textContent = PLACEHOLDER;
          name.then(
            (resolved) => {
              if (now === asked) dom.textContent = shown(resolved);
            },
            () => {
              if (now === asked) dom.textContent = PLACEHOLDER;
            },
          );
        } else {
          dom.textContent = shown(name);
        }
      };
      show(node.attrs.space, node.attrs.id);
      return {
        dom,
        update: (next) => {
          if (next.type !== node.type) return false;
          if (
            next.attrs.space !== node.attrs.space ||
            next.attrs.id !== node.attrs.id
          ) {
            show(next.attrs.space, next.attrs.id);
          }
          node = next;
          return true;
        },
        ignoreMutation: () => true,
      };
    };
  },
});

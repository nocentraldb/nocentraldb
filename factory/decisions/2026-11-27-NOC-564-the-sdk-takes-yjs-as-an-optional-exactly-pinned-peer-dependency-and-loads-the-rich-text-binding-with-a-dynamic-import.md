# 2026-11-27 · NOC-564 · The SDK takes yjs as an optional, exactly pinned peer dependency, and space.richText loads the rich-text binding with a dynamic import

- Decided by: Carsten, 2026-11-27, on NOC-564, through pm
- Why: the editor's y-tiptap and the SDK's binding must share one Yjs, as two copies break Yjs's own checks, so the app installs it once; only rich text needs it, so it is optional, and the binding that imports it is loaded only when space.richText opens one, which keeps nc.open and every other call working without Yjs, also unbundled.
- Instead of: yjs as a dependency of the SDK, which forces one exact Yjs on every app and gives an editor that wants another a second copy; a static import, which makes a bundler or an import map mandatory for every app.
- Topics: rich-text, sdk-ts, dependencies, packaging

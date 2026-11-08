# 2026-11-08 · NOC-33 · The probe page goes to GitHub Pages only when a person starts it, on one path

- Decided by: coding agent (NOC-33), on pm's request; enabling Pages stays with the owner
- Why: publishing a page is the owner's call, so `browser-storage-pages.yml` runs only on `workflow_dispatch` and does nothing until Pages is enabled. A Pages project site is one origin (`https://nocentraldb.github.io`) whatever the path, so a path per check would not separate storage; the checklist separates checks by browser and app instead.
- Instead of: deploying on every push to `main`, or one path per check, which would suggest a separation it does not give.
- Topics: browser-storage, ci, hosting

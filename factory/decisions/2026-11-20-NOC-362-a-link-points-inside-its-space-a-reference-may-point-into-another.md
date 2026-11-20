# 2026-11-20 · NOC-362 · A link points inside its space; a reference (nc.ref) may point into another

- Decided by: Carsten, 2026-11-20, on NOC-362 ("Ja zu den Empfehlungen")
- Why: RFC 0004 and RFC 0005 already keep a link inside its object's space and make the reference (`nc.ref`, Rust `Reference { space, id }`) the field that may point into another space (NOC-63, NOC-354). The glossary said a link may point into another space; it now follows the RFCs and has a row for the reference.
- Instead of: links that cross spaces, which the glossary said and no RFC decided.
- Topics: glossary, links, references

# 2026-11-22 · no ticket · Research notes stay out of the repository; their findings go into the RFC

- Decided by: Carsten, 2026-11-21, in chat with pm, on #435 ("es ist nur ein Recherche-Ergebnis oder? Warum heben wir es auf? Genügt es nicht, die Erkenntnisse in dem RFC aufzunehmen?"), confirmed with "Mach es so"
- Why: a research note or a spike write-up is a step towards a decision, not a record of one. Once the decision is made, the RFC states the design with its reasons and the options not taken, and the decision log says who decided. A separate note in the repository then repeats or contradicts the RFC, and agents who read it may take a superseded finding for the design. So the findings of research and spikes go into the RFC, as the reasons for its choices, or into the ticket as a comment; open points go to the owner as a needs-owner ticket. Spike code is kept only where a build ticket reuses it as a test or fixture, in that ticket's test suite.
- Instead of: keeping research notes under `docs/research/` or beside the spike code.
- Topics: rfc, research, spikes, factory

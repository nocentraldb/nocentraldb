# Landing page: lead with speed, explain the product in one scroll scenario

- Date: 2026-11-12
- Owner: Carsten (every decision below marked "owner" is his, made on 2026-11-12)
- Status: approved by the owner on 2026-11-12 (the proposal, with the colour decision of the same day)
- Scope: `site/` (the landing page at nocentraldb.io). The docs under `/docs` do not change.

## Intent and why

A developer who lands on nocentraldb.io today does not learn how the product works. The hero speaks in the product's own terms (copy, keys, relay), the main benefit sits in one of eight cards, and no picture shows a device, an agent and the server between them. The page also looks flat and rigid: every section has the same padding and the same card grid, and some gaps are broken.

After the rework a developer should know, from the first two screens:

1. It is a local-first database.
2. People and agents work on the same data, each on a full local copy.
3. The copies sync through a server that cannot read the content.
4. So agents work on the data at local speed, and work goes on when the connection is gone.

## Goals

- The hero says what the product is in words a stranger understands.
- Speed leads, confidentiality follows (owner).
- One scroll scenario explains the mechanism step by step: a laptop on a train, a desk in a home office, an agent on its own machine, and a server in the cloud between them (owner).
- The page looks made for this product: one section per screen, equal edges, a hero entrance, hover feedback.

## Non-goals

- No limits section and no FAQ in this version (owner). The limits stay in the docs.
- No change to the docs, to Starlight or to `llms.txt`.
- No real SDK running in the page. The scenario is a drawing.
- No new brand work: logo, colours and typefaces stay as the brand defines them.

## Anti-goals

- A claim the design does not support: invented benchmarks, milliseconds, users or partners.
- Anything described as built. The status line stays (brand claims rule, status).
- Statements about erasure. The brand requires each one to name its limits, and the limits are out of this version, so shredding does not appear on the page.
- The agent drawn so that it can be mistaken for the server in the cloud.

## Messages and copy

Headline (owner): **A database that reads like a local file and syncs like a shared doc**

| Place | Text |
| --- | --- |
| Eyebrow | A local-first database for people and agents |
| Subline | Each person and each agent reads and writes a full copy on their own machine. The changes sync encrypted through a server that can't read them. |
| Primary button | See how it works (jumps to the scenario) |
| Secondary button | Review the RFCs |
| Status line | NoCentralDB does not exist yet. This page shows what it is designed to do; the design is open in the RFCs. |
| Fact line (my call) | SDKs for TypeScript and Rust. One core for desktop, mobile and the browser. Open source under MIT or Apache-2.0. |
| Page title | NoCentralDB: a local-first database for people and agents (unchanged) |
| Meta description | The subline |

The scenario, heading "One task list on a train, in a home office and with an agent":

| Step | Title | Text |
| --- | --- | --- |
| 1 | Everyone holds a full copy | The laptop on the train, the desk at home and the agent each keep the whole task list on their own disk. The agent reads its copy as often as it likes, and no read crosses the network. |
| 2 | A change travels as ciphertext | The train marks a task as in progress. The change goes through a server in the cloud to the other two. Hold a lens over that server and you see bytes it has no key for. |
| 3 | The train enters a tunnel | The laptop is offline and its owner keeps working; the change is stored locally. At home and with the agent the work goes on too. They don't see the train's change yet. |
| 4 | Out of the tunnel, every copy matches | The laptop reconnects. Its change goes out, the others' changes come in, and all three copies end in the same state. |

The fact line is new and not in the prototype. The design skill's copy check wants one hard fact on the first screen (a number, a date, a place or a platform), and the anti-goals rule out numbers, so the hero names the platforms the design targets. It sits with the status line, which says that none of it is released. If the check still fails on the owner's headline or subline, the builder records a waiver with the owner's decision as the reason; the headline and subline do not change.

The copy is a draft the owner has seen in the prototype. Before the PR it goes through the copy-editing pass of the copywriting skill and the copy check of the design skill.

**Brand conflict the owner has decided.** The brand guideline (nxsflow/brand, "NoCentralDB (endorsed)", visible lines) prescribes the headline "For when a central database is the wrong place." and the subline "Everyone works on their own copy, under their own keys. The relay in the middle can't read a thing." The owner replaced both. The guideline has to follow; that change belongs to the brand repository and is not part of this work.

## The page

Four sections, each one screen tall at 1440x900 except the scenario, which is two.

| # | Section | Content |
| --- | --- | --- |
| 1 | Hero | Eyebrow, headline, subline, two buttons, status line at the left. At the right the scenario's first picture, at rest, with the agent's local reads moving. |
| 2 | Scenario | The four steps above. The picture stays in place while the visitor scrolls; the steps open one at a time and the picture changes with them. |
| 3 | Code | The five SDK calls in tabs, with the install line. The example changes from the chat to the scenario's task list (my call): the model is the `people` and `tasks` model of the README, the calls stay the five of RFC 0009 (model, open, share, write, subscribe). The first note that says "relay" explains it once as the server from the scenario. |
| 4 | Close | "Read the design, then tell us where it breaks", the four doc links, the feedback button, and one link to the use cases (`docs/use-cases.md` on GitHub, since the site has no page for them). Footer with the tagline, the licence line and "Initiated by nxsflow." |

Removed: the use-case cards (their three strongest points live in the scenario; the rest is the link in the close), the simulated chat ("Two members, one space"), the limits section, and the hero's member-and-relay panel.

Header: the NoCentralDB wordmark (owner), links "How it works", "Code", "Docs", "GitHub".

## The scenario picture

One drawing, 4:3, in four states. Top centre: a cloud labelled "Cloud server" that shows two short rows of ciphertext. Below it three cards in a row, each a copy of the same three tasks: "On the train", "Home office", "Agent". Under each card its place: a train on a track, a house, a machine in a rack. A hill with a tunnel mouth stands ahead of the train from the start.

| State | What changes |
| --- | --- |
| 1 | Three identical copies. A point circles the agent's card: its reads, which stay local. No point travels on the lines to the cloud. |
| 2 | The train's first task turns yellow ("in progress"). A point travels from the train to the cloud and on to the other two, whose task gets a yellow outline. A lens over the cloud shows large ciphertext. |
| 3 | The train's front is inside the tunnel mouth. Its line to the cloud is dotted and its state mark is hollow. Its second task is yellow ("done · stored locally"). The home office changes the third task; that change travels to the agent. The train's copy lacks it, the others lack the train's. |
| 4 | The hill is behind the train. Points travel both ways on the train's line. All three copies show the same three tasks; the rows that just arrived carry the outline. |

Yellow marks data that changes: the changed row and the travelling point. Magenta marks a live connection: the state mark in each card's header.

The wording inside the drawing avoids the product's own terms: "Cloud server", never "relay", because the scenario is where a stranger first meets the mechanism. The terms space, relay and island mode appear in the docs links.

A prototype of the hero and the scenario exists and the owner has accepted its content, order, look and tunnel. Its generator is kept beside this document as `2026-11-12-landing-page-prototype/build.mjs`: it holds the drawing's geometry, classes and colours and the four states. It is the reference for the picture; the builder ports the drawing into an Astro component and removes the prototype folder in the same PR. To see it, copy the kit files it links (`assets/system.css`, `ui/`, `motion/`, `backgrounds/`, `layout/`, `moments/` from the design skill, and the wordmark) next to it, run `node build.mjs` and serve the folder over http.

## Approaches

How the scenario is built:

- **A. The design skill's pinned-chapters building block, four steps (chosen by the owner).** Two screens of scroll, the drawing as an SVG of our own in four states that cross-fade. The block limits the scenario to four steps, which forced the condensation above. In return it brings the resting version for phones and for reduced motion, and the design skill's layout and motion checks pass without exceptions (the copy check: see "Messages and copy").
- **B. A scroll scene of our own, six steps.** Closest to the first description: read, write, lens, entering the tunnel, working in it, merging, each its own step over four to five screens, with the train moving. It breaks the skill's rule on section length, needs documented exceptions in its checks, and the phone version would be ours to build.
- **C. The same scene without scroll coupling.** One screen, a loop with "next" buttons. Simplest, but it is not the scroll experience the owner asked for.

How the page is built:

- **The design skill's kit inside the existing Astro site (my call).** The landing page moves onto the kit's layout, type scale, buttons, navigation and motion layer, with the NoCentralDB brand tokens and typefaces. The kit's files it needs are copied into `site/public/assets/` and loaded as plain files, as the skill prescribes for an existing project. The kit is MIT-licensed (github.com/Leonxlnx/taste-skill); the copy carries its licence notice.
- Not taken: keeping the hand-written `landing.css` and adding only the scenario. The owner's complaint is the whole page's rigidity and spacing, and the building block expects the kit's section box and grid.

## Colour

Two colours carry the page, and a third appears only inside the scenario's drawing (owner, after the design skill's assessment):

| Colour | Job |
| --- | --- |
| Near-black | The ground of the hero, the scenario and the code section, and every panel on them. |
| Yellow, the accent | The primary buttons, the focus ring, link underlines, the dot in the eyebrow, and the ground of the close, flooded edge to edge with near-black cards on it. On that ground the primary button is near-black. In the drawing it marks data: the changed row and the change that travels. |
| Magenta #FF3D8B, the loud colour | Only inside the drawing, in the hero and in the scenario: the "connected" mark of each member. Nowhere else on the page: no button, no link, no ground, no text. |

In the drawing the two colours mean two things: yellow is data that changes, magenta is a live connection. A member that is offline shows a hollow mark in the muted tone.

This follows the design skill: an interface that is 90 to 95 % neutral, one accent, and one section in the brand's own colour, once, in the second half of the page. The magenta marks sit inside a visual, where the skill allows a state colour. If its checker still reports them as a second accent, the builder records a waiver with the owner's decision as the reason.

It supersedes the decision `2026-11-11-NOC-211-the-landing-page-is-60-30-10-with-hot-magenta.md`, which gave magenta the primary action, link underlines and focus and about 30 % of the page to yellow. The build records a new decision file that names it in `Supersedes:`. The docs under `/docs` keep their current colours; aligning them is not part of this work.

## Constraints

- Stack: Astro 7 in `site/`, its own pnpm project. The build stays static; `site/scripts/output.test.mjs` and the `repo:site` gate must stay green.
- Brand: NoCentralDB tokens from `site/src/styles/tokens.css`, dark theme only, Encode Sans Semi Expanded, Commissioner, Syne Mono. No yellow in code highlighting. No gradients, no glow at rest.
- Design skill: one section per screen, equal edges, the hero alone on the first screen, a pause control for anything that moves longer than five seconds, a still version under reduced motion and on phones. Its checks run at 1440, 1024, 820, 768, 390 and 320.
- The hero must fit a short desktop window (seen at 1440x670): the headline scales with the window height.
- Claims: "designed to" until the first release; no numbers that were not measured.
- Process: the landing page is something the owner sees first (GENESIS, "What has to come back through me"). The work runs as one ticket in the tracker and one draft PR that the owner reviews.
- File size: no code file over 700 lines (the gate's `repo:file-size`); the drawing's states come from one function, not four hand-written SVGs.

## Left to the builder

- The exact geometry of the drawing, the timing of the travelling points and the cross-fade.
- How the kit's navigation is cut down to four plain links.
- Which of the kit's files are copied, as few as the page needs.
- The phone layout of the code tabs.
- The tightened copy of the code and close sections, within the messages above.
- Whether the old components are deleted or rewritten in place.

## After the page: the brand follows

The owner wants the brand description and the page to agree again. Once this design is approved, the NoCentralDB section of the brand guideline (repository `nxsflow/brand`, BRAND.md, "NoCentralDB (endorsed)") is updated in that repository, as its own piece of work under that repository's rules:

- Visible lines: the new headline and subline replace the old ones; eyebrow and tagline stay.
- Colours: yellow stays the one accent; the rule "the accent takes under 5 % of any screen" gives way to one flooded section per page. Hot magenta is added as the role "loud", allowed only inside animated drawings, for the connected state.
- The nxsflow-brand skill and the headline skill's list of rejected lines follow the guideline.

`site/README.md` names the brand commit the page follows; it is updated to the new commit when that lands.

## Decided by the owner on 2026-11-12

- Speed leads, confidentiality follows; no limits section and no FAQ in this version.
- The headline, the scenario with the agent as third member on its own machine, approach A, the wordmark in the header, the tunnel as drawn in the prototype.
- Two colours on the page, near-black and yellow, with one flooded yellow section; magenta only inside the drawing.
- The agent's speed is shown by step 1 of the scenario and the point circling its copy; the hero picture is the scenario's first state.

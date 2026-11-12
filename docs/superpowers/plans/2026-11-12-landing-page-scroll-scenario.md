# Landing Page Scroll Scenario Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the landing page at nocentraldb.io as four sections (hero, scroll scenario, code, close) that lead with speed and explain the product through one drawing in four states.

**Architecture:** The page stays a static Astro page in `site/`. It moves onto the design skill's kit (layout, type scale, buttons, motion layer, the `pinned-chapters` moment), vendored into `site/public/assets/` and loaded as plain files. The drawing is one pure function that returns an SVG string per state; the hero and the scenario both call it.

**Tech Stack:** Astro 7, plain CSS, inline SVG with SMIL motion, `node:test` on the built output, the design skill's checkers (`.claude/skills/design-taste-frontend/scripts/`).

**Spec:** `docs/superpowers/specs/2026-11-12-landing-page-scroll-scenario-design.md`. Read it first; this plan argues from it. The accepted prototype is `docs/superpowers/specs/2026-11-12-landing-page-prototype/build.mjs`.

## Global Constraints

- One ticket in Linear team `NOC`, one branch (Linear's branch name for it), one draft PR. The owner reviews it; nobody else ticks its Authorship box. The ticket has to exist before Task 1; its id is written `NOC-n` below.
- English for everything durable. Commit subjects start with the ticket id, as the repository's log shows (`NOC-n: ...`).
- The design skill is installed only locally (`.claude/skills/` is git-excluded on this machine). The work runs where the skill is installed. `SKILL` below means `.claude/skills/design-taste-frontend`.
- Copy is fixed by the spec's tables ("Messages and copy"): headline, eyebrow, subline, buttons, status line, fact line, the scenario's heading and its four titles and texts. Use them verbatim.
- Claims: "designed to" until the first release; no measured-looking numbers; no statement about erasure; "relay" does not appear in the hero or the scenario.
- Colour: near-black and yellow on the page; the close is flooded yellow with a near-black primary button; magenta `#FF3D8B` only inside the drawing, for the connected mark. No yellow in code highlighting, no gradients, no glow at rest.
- Brand tokens come from `site/src/styles/tokens.css`, which stays an unchanged copy of the brand's file. Typefaces: Encode Sans Semi Expanded 700, Commissioner 350/500/700, Syne Mono.
- `pnpm gates` must be green, which includes `repo:site` (`pnpm --dir site build && node --test 'site/scripts/*.test.mjs'`) and `repo:file-size` (no `.js`/`.mjs`/`.ts` file over 700 lines; the vendored kit's script files are all under it).
- Never edit a vendored kit file to clear a check. A check judged false is waived in `.taste-skill/waivers.json` with its reason.
- The docs under `/docs`, Starlight and `llms.txt` do not change.

## Review Focus

1. **A short desktop window (1440x670).** The hero's status and fact lines must end inside the first screen, and the pinned scenario must fit. Pinned by the layout check at that size in Task 6.
2. **No script, reduced motion, a phone.** All four steps, with their texts and drawings, are in the HTML and readable when nothing is pinned. Pinned by the output test in Task 3 (four titles, four texts, four drawings in the built HTML) and the checker at 390 in Task 6.
3. **Two drawings of the same state on one page.** The hero and step 1 both show state 0; SVG ids (`clipPath`) must not collide. Pinned by the id test in Task 2.
4. **Motion that never stops.** The drawing's points loop for ever; the page needs a pause control that stops them, and they must rest under reduced motion. Pinned by the checker's `loop-pause` and `reduced-motion` in Task 6 and the markup test in Task 3.
5. **Keyboard on the code tabs.** Arrow keys move between tabs and the panel follows, as today. Pinned by the output test in Task 4 (roles and `aria-controls` pairs) and a manual pass in Task 6.

---

### Task 1: The page shell on the kit

**Files:**
- Create: `site/public/assets/` with the kit files the page needs, copied unchanged from `SKILL/assets/`: `system.css`, `system.js`, `ui/cards.css`, `motion/` (its `.css` and `.js`), `layout/layout.css`, `layout/layout.js`, `moments/moments.js`, `moments/lib.js`, `moments/pinned-chapters/moment.js`, `moments/pinned-chapters/moment.css`; plus `site/public/assets/LICENSE-taste-skill.md` (the MIT text of github.com/Leonxlnx/taste-skill and the version, tasteskill 2.5.1)
- Create: `site/src/styles/kit-tokens.css`
- Create: `site/scripts/landing.test.mjs`
- Modify: `site/src/layouts/Landing.astro`, `site/biome.json` (leave `public/assets/` out of linting)
- Delete: `site/src/styles/landing.css` once nothing imports it (Task 4 at the latest)

**Interfaces:**
- Produces: the shell every section sits in: `<html lang="en" data-type="software" data-radius="soft" data-buttons="pill" data-motion>`, the metas `site-kind` and `page-type` (both `software`), `header.nav > .wrap.nav__bar`, `main#top`, `footer.closing`. Sections are `main > section` with one `.wrap` child.
- Produces: `kit-tokens.css` sets the kit's `--p-*`, `--font-display`, `--font-text`, `--font-mono`, `--display-weight: 700`, `--display-tracking: -0.015em` from the `--ncdb-*` roles (`--p-action` is the accent, `--p-action-ink` the accent ink).
- Produces: `site/scripts/landing.test.mjs` exports nothing; it defines `const html = readFileSync("site/dist/index.html", "utf8")` for the tests later tasks add.

- [ ] **Step 1: Write the failing test** in `site/scripts/landing.test.mjs`:

```js
test("the page declares its kind and loads the kit", () => {
  assert.match(html, /<html[^>]*data-type="software"[^>]*data-motion/);
  assert.match(html, /<meta name="site-kind" content="software"/);
  assert.match(html, /href="\/assets\/system\.css"/);
  assert.match(html, /src="\/assets\/motion\/motion\.js"/);
});
test("the header shows the wordmark and four links", () => {
  assert.match(html, /<svg[^>]*role="img"[^>]*aria-label="NoCentralDB"/);
  for (const label of ["How it works", "Code", "Docs", "GitHub"]) assert.ok(html.includes(`>${label}<`), label);
});
test("the title is unchanged and the description is the subline", () => {
  assert.match(html, /<title>NoCentralDB: a local-first database for people and agents<\/title>/);
  assert.ok(html.includes("Each person and each agent reads and writes a full copy on their own machine."));
});
```

- [ ] **Step 2: Run it and see it fail.** `pnpm --dir site build && node --test site/scripts/landing.test.mjs` → FAIL on `data-type`.
- [ ] **Step 3: Vendor the kit and rewrite `Landing.astro`** to the shell above. Stylesheet order as `SKILL/assets/starter.html` gives it (system, cards, motion, layout), then `tokens.css`, then `kit-tokens.css`. Keep the inlined wordmark, the favicon links, the font preconnects and the `?shot` switch. The nav is the kit's bar cut down to four plain links (no menu panels); the `m-enter` and `mo-js` head lines of the starter stay. Sections come in Tasks 3 and 4; until then `main` holds the old components.
- [ ] **Step 4: Arm the design skill's gate.** `pnpm --dir site build && node SKILL/scripts/gate.mjs --start site/dist/index.html --root site/dist --kind software --category "local-first database" --primary "See how it works"`.
- [ ] **Step 5: Run the test and `pnpm --dir site exec biome check .`** → PASS, no lint finding from `public/assets/`.
- [ ] **Step 6: Commit.** `NOC-n: the landing page shell on the design kit`

### Task 2: The drawing

**Files:**
- Create: `site/src/components/scene.mjs`, `site/src/styles/scene.css`
- Test: `site/scripts/scene.test.mjs`

**Interfaces:**
- Produces: `export const STEPS` (four objects `{ title, text, alt }`, titles and texts from the spec's scenario table, `alt` the one-sentence description of the state) and `export function scene(step, idPrefix)` → an `<svg class="sc" viewBox="0 0 800 600" role="img" aria-label="…">` string. `step` is `0 | 1 | 2 | 3`; `idPrefix` is a string that starts every `id` inside the SVG.
- Colours live in `scene.css` as classes on the SVG's parts (the prototype's `.sc-*` classes), reading the `--ncdb-*` roles; magenta is the one literal, `--loud: #ff3d8b`, declared in `scene.css` and used by `.sc-dot-on` only.

Port the prototype's `card`, `cloud`, `train`, `hill`, `scenery`, `link` and `pulse` functions and its four states. The spec's table "The scenario picture" says what each state shows.

- [ ] **Step 1: Write the failing tests** in `site/scripts/scene.test.mjs`:

```js
import { STEPS, scene } from "../src/components/scene.mjs";
const count = (s, needle) => s.split(needle).length - 1;

test("four steps with the spec's titles", () => {
  assert.deepEqual(STEPS.map((s) => s.title), [
    "Everyone holds a full copy",
    "A change travels as ciphertext",
    "The train enters a tunnel",
    "Out of the tunnel, every copy matches",
  ]);
});
test("state 0: three equal copies, the agent's reads circle, nothing travels to the cloud", () => {
  const s = scene(0, "a");
  assert.equal(count(s, ">open<"), 9);
  assert.equal(count(s, "<animateMotion"), 1);
  assert.equal(count(s, 'class="sc-lens"'), 0);
});
test("state 1: the lens is on the cloud and the change reaches both others", () => {
  const s = scene(1, "a");
  assert.equal(count(s, 'class="sc-lens"'), 1);
  assert.equal(count(s, "sc-row-origin"), 1);
  assert.equal(count(s, "sc-row-got"), 2);
});
test("state 2: the train is offline and its change is stored locally", () => {
  const s = scene(2, "a");
  assert.equal(count(s, "sc-dot-off"), 1);
  assert.equal(count(s, "sc-link-cut"), 1);
  assert.ok(s.includes("stored locally"));
});
test("state 3: all three copies show the same three tasks", () => {
  const s = scene(3, "a");
  for (const status of [">in progress<", ">done<", ">assigned to agent<"]) assert.equal(count(s, status), 3, status);
  assert.equal(count(s, "sc-dot-off"), 0);
});
test("the drawing never says relay", () => {
  for (const i of [0, 1, 2, 3]) assert.ok(!/relay/i.test(scene(i, "a")));
});
test("ids carry the prefix, so two drawings can share a page", () => {
  const ids = (s) => [...s.matchAll(/\bid="([^"]+)"/g)].map((m) => m[1]);
  for (const i of [0, 1, 2, 3]) {
    assert.ok(ids(scene(i, "hero")).every((id) => id.startsWith("hero")));
    assert.equal(ids(scene(i, "a")).filter((id) => ids(scene(i, "b")).includes(id)).length, 0);
  }
});
```

- [ ] **Step 2: Run and see them fail.** `node --test site/scripts/scene.test.mjs` → FAIL, module not found.
- [ ] **Step 3: Implement `scene.mjs` and `scene.css`.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit.** `NOC-n: the scenario drawing in four states`

### Task 3: Hero and scenario

**Files:**
- Modify: `site/src/components/Hero.astro` (rewrite), `site/src/layouts/Landing.astro`
- Create: `site/src/components/Scenario.astro`
- Delete: `site/src/components/UseCases.astro`, `site/src/components/LiveExample.astro`
- Test: `site/scripts/landing.test.mjs`

**Interfaces:**
- Consumes: `STEPS`, `scene(step, idPrefix)` from Task 2; the shell from Task 1.
- Produces: `section.hero.hero--split[data-enter]` with `#hero-h`; `section#story[data-moment="pinned-chapters"][style="--h: 2"]`, its markup copied from `SKILL/assets/moments/_demo/in-page-service.html` (`.mo-pc__grid`, `.mo-pc__steps`, `.mo-pc__step`, `figure.mo-pc__visual`, `.mo-pc__visuals.frame`).

The hero's picture is `scene(0, "hero")` in a 4:3 frame; the scenario's four figures are `scene(i, "s" + i)`. The primary button links to `#story`, the secondary to the RFC index on GitHub (the live page's URL). One pause control (`button[data-scene-pause]`, text "Pause" / "Play", `aria-pressed`) sits in each drawing's frame and calls `pauseAnimations()` / `unpauseAnimations()` on the SVGs; under `prefers-reduced-motion: reduce` and with `?shot` the SVGs start paused and the control is hidden. The headline's size follows the window height on short desktop windows (the prototype's rule: `clamp(36px, 7.6svh, 60px)` under 780px of height from 1024px of width).

- [ ] **Step 1: Write the failing tests** (append to `landing.test.mjs`):

```js
import { STEPS } from "../src/components/scene.mjs";

test("the hero says what it is, in the spec's words", () => {
  assert.match(html, /<h1[^>]*>A database that reads like a local file and syncs like a shared doc<\/h1>/);
  for (const line of [
    "A local-first database for people and agents",
    "The changes sync encrypted through a server that can't read them.",
    "NoCentralDB does not exist yet.",
    "SDKs for TypeScript and Rust.",
    "See how it works",
    "Review the RFCs",
  ]) assert.ok(html.includes(line.replace("'", "&#39;")) || html.includes(line), line);
});
test("the scenario holds all four steps and five drawings in the HTML", () => {
  assert.match(html, /<section[^>]*id="story"[^>]*data-moment="pinned-chapters"/);
  for (const s of STEPS) assert.ok(html.includes(s.title), s.title);
  assert.equal(html.split('class="sc"').length - 1, 5);
  assert.equal(html.split("mo-pc__step\"").length - 1, 4);
});
test("hero and scenario never say relay, and the drawings can be paused", () => {
  const upTo = html.indexOf('id="code"');
  assert.ok(!/relay/i.test(html.slice(html.indexOf("<main"), upTo)));
  assert.ok(html.includes("data-scene-pause"));
});
test("the removed sections are gone", () => {
  for (const id of ['id="use-cases"', 'id="live"']) assert.ok(!html.includes(id), id);
});
```

- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Build the hero and the scenario**, add them to `Landing.astro`, and remove the two old components. The limits section goes in Task 4 with `Closing.astro`.
- [ ] **Step 4: Run** → PASS, and look at it: `pnpm --dir site preview`, scroll the scenario at 1440x900 and 1440x670.
- [ ] **Step 5: Commit.** `NOC-n: the hero and the scroll scenario`

### Task 4: Code and close

**Files:**
- Modify: `site/src/components/CodeSection.astro`, `site/src/components/Closing.astro`, `site/src/layouts/Landing.astro`, `site/scripts/screenshots.mjs`
- Delete: `site/src/styles/landing.css` if still present
- Test: `site/scripts/landing.test.mjs`

**Interfaces:**
- Consumes: the shell from Task 1.
- Produces: `section#code` (one screen) and `footer.closing` holding the close on the yellow ground (`.ground-brand`) and the footer row.

Code: heading "Five calls in the SDK"; lead "No keys in your code: the SDK is designed to create, store and rotate them."; the install line and the Rust line as today; five tabs Model, Open, Share, Write, Subscribe. The snippets are the README's (`README.md`, "Code examples"): the `tasks` collection with `byStatusPriority`, `nc.open`, the invitation and `members.add`, `project.write` with "Write the README", and `byStatusPriority.equal("open").take(50)` with `db.subscribe`. The Open tab's note is the first to say relay and explains it: "The relay is the server from the scenario above. It needs no relay to start." Tabs keep `role="tablist"`, arrow keys and the copy button; the kit's tab styles replace the hand-written ones where they exist.

Close: heading "Read the design, then tell us where it breaks"; the four doc cards with today's texts and links; the button "Share your feedback" (near-black on the yellow ground) to the GitHub discussions; one link "See the use cases" to `https://github.com/nocentraldb/nocentraldb/blob/main/docs/use-cases.md`. Footer: "No center. No collisions.", "Open source under MIT or Apache-2.0. Initiated by nxsflow.", the GitHub link.

`screenshots.mjs`: file names `landing-desktop.png` and `landing-mobile.png`, and a third shot at 1440x670; the header comment no longer speaks of magenta.

- [ ] **Step 1: Write the failing tests:**

```js
test("the code section shows the task model in five tabs", () => {
  const code = html.slice(html.indexOf('id="code"'));
  assert.equal(code.split('role="tab"').length - 1, 5);
  for (const m of code.matchAll(/role="tab"[^>]*aria-controls="([^"]+)"/g)) assert.ok(code.includes(`id="${m[1]}"`), m[1]);
  assert.ok(code.includes("byStatusPriority"));
  assert.ok(!code.includes("nc.collection(\n    {\n      text"), "the chat model is gone");
  assert.ok(code.includes("The relay is the server from the scenario above."));
});
test("the close sits on the yellow ground and links to the docs and the use cases", () => {
  assert.match(html, /<footer[^>]*class="[^"]*closing/);
  assert.ok(html.includes("Read the design, then tell us where it breaks"));
  for (const href of ['href="/docs/"', 'href="/docs/api/"', 'href="/docs/rfcs/"', "docs/use-cases.md", "/discussions"]) assert.ok(html.includes(href), href);
  assert.ok(html.includes("Initiated by nxsflow."));
  assert.ok(!html.includes('id="limits"'), "the limits section is gone");
});
test("magenta appears only in the drawing's stylesheet", async () => {
  const { readdirSync } = await import("node:fs");
  assert.ok(!/ff3d8b/i.test(html));
  const css = readdirSync("site/dist/_astro").filter((f) => f.endsWith(".css")).map((f) => readFileSync(`site/dist/_astro/${f}`, "utf8")).join("");
  assert.equal(css.split(/ff3d8b/i).length - 1, 1);
});
```

- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Rewrite the two components** and remove what is left of the old page's styles.
- [ ] **Step 4: Run the whole site suite** `pnpm --dir site build && node --test 'site/scripts/*.test.mjs'` → PASS.
- [ ] **Step 5: Commit.** `NOC-n: the code section on the task model and the yellow close`

### Task 5: Copy pass

**Files:**
- Modify: the four components, only where a check or the pass finds something outside the spec's fixed copy

- [ ] **Step 1: Run the copy check.** `node SKILL/scripts/copy-lint.mjs site/dist/index.html --kind software`.
- [ ] **Step 2: Fix every FAIL and WARN in copy the spec leaves to the builder** (code notes, card texts, link words). A finding on the headline, subline, status line or the scenario's texts is not fixed: it is waived in `.taste-skill/waivers.json`, reason "owner's copy, decided 2026-11-12".
- [ ] **Step 3: Run the copy-editing skill's pass over the page's text** and apply only edits that remove an AI tell or an error; the spec's fixed lines stay.
- [ ] **Step 4: Rebuild, rerun the site suite** → PASS.
- [ ] **Step 5: Commit.** `NOC-n: copy pass on the landing page`

### Task 6: The design checks, the decision and the hand-over

**Files:**
- Create: `factory/decisions/2026-11-12-NOC-n-the-landing-page-leads-with-speed-in-two-colours.md`, `.taste-skill/waivers.json` (only if a waiver is needed; `.taste-skill/` history files stay out of git)
- Modify: `site/README.md` (the Brand paragraph: two colours and the drawing's magenta, the new decision file)
- Delete: `docs/superpowers/specs/2026-11-12-landing-page-prototype/`, and the spec and this plan once the decision file and the PR body carry their content

The decision file, in FACTORY.md's format: decided by Carsten, 2026-11-12; why: the page leads with speed, explains the mechanism in one scroll scenario, uses near-black and yellow with one flooded section and magenta only inside the drawing; instead of: 60/30/10 with magenta as the action colour, a limits section, the brand's former headline; topics: site, brand, design; `Supersedes: 2026-11-11-NOC-211-the-landing-page-is-60-30-10-with-hot-magenta.md`.

- [ ] **Step 1: Run the checks**, one at a time, in the foreground, each with `--profile .taste-skill/chrome-check`:

```bash
pnpm --dir site build
node SKILL/scripts/consistency.mjs site/dist/index.html --root site/dist --widths 1440,1024,820,768,390 --rhythm-widths 1440,820,768,390 --profile .taste-skill/chrome-check --page software --category "local-first database" --primary "See how it works"
node SKILL/assets/layout/check.mjs --url site/dist/index.html --root site/dist --profile .taste-skill/chrome-check --sizes 1440x900,1440x670,1024x768,820x1180,768x1024,390x844 --shots site/drafts/layout
node SKILL/scripts/perf.mjs site/dist/index.html --root site/dist --profile .taste-skill/chrome-check
```

- [ ] **Step 2: Fix every FAIL.** Expected to need a waiver, with the owner's decision as the reason: a second-accent finding on the drawing's magenta, and a finding on the hand-chosen hero, buttons and rhythm (the owner accepted the prototype, so they were not dealt). Anything else is fixed in the page, never in the kit.
- [ ] **Step 3: Look.** Shoot 1440, 1440x670, 1024, 820x1180, 768x1024 and 390 with `SKILL/scripts/shoot.mjs` and read each against the skill's checklist; tab through the page once (skip link, nav, buttons, the five code tabs with arrow keys, the pause control).
- [ ] **Step 4: Run the gate until it says PASS or OPEN**, three rounds at most: `node SKILL/scripts/gate.mjs site/dist/index.html`.
- [ ] **Step 5: Write the decision file, update `site/README.md`, remove the prototype folder, the spec and this plan.**
- [ ] **Step 6: Run `pnpm gates`** → green. If `repo:fragments` asks for a change fragment, `docs/release-management.md` says whether `site/` needs one; follow it.
- [ ] **Step 7: Commit, push, open the draft PR** with the screenshots at 1440, 1440x670 and 390, the gate line, every waiver with its reason, and what was not checked. Leave the Authorship box for the owner.

## After this plan

The brand guideline follows the page (spec, "After the page: the brand follows"). That is separate work in `nxsflow/brand` under that repository's rules, and its local checkout has to be updated first: its `main` holds no NoCentralDB section today.

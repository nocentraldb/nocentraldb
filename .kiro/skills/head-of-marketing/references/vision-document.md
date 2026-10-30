# Playbook: the vision and call-for-feedback document

Use this when a project needs a public document BEFORE a product exists: it states a vision,
describes concrete use cases, and asks a community for feedback — an early, RFC-like piece. The
same rules serve a launch or announcement text that has to explain a new thing quickly.

THE GOAL: a reader understands within seconds what the product does for them, why it is needed,
and why it is worth reading on. Everything below serves that goal; anything in a draft that does
not is cut.

How the rules are cited: each rule names the area it comes from, e.g. *(positioning; KOB ch. 1)*.
The full reasoning is in `references/knowledge/<area>.md`; read the file when you need the reasoning
behind a rule or when a draft hits a case the rule does not settle. The project's own decisions
(naming rules, tone, an existing positioning paper) beat every rule here — SKILL.md, "What the
project has already decided beats the book". Before you write, establish the facts and write the
dossier as SKILL.md says; the product content comes from the project, never from this file.

---

## 1. Decide two things before any sentence is written

### Who it is for — a smallest viable audience, not "developers"
- Choose readers by what they believe, want and are frustrated by, not by job title alone — a
  worldview, not a demographic *(audience; TIM ch. 4)*. "Backend developers" is a demographic;
  "developers who are tired of their app being useless the moment the network drops" is a
  worldview.
- Aim at the smallest group that would make it worth the effort and that talks to each other. A
  focused core spreads the word to the next layer; "everyone" spreads nothing *(audience; TIM
  ch. 4, ch. 9)*. A document written for everyone offends no one and moves no one.
- Write down who it is NOT for. "It's not for you" respects the reader's time and is a positioning
  discipline, not a loss *(positioning; TIM ch. 4)*. The fear of excluding someone is the usual
  reason vision texts stay vague *(positioning; KOB ch. 9)*.
- Separate user, buyer and decider. In technology markets the person who would use it is often not
  the one who approves it; the document must also give the user a story to tell upward *(b2b; KOB
  ch. 9, TIM ch. 19)*.

### The one change it promises
- Marketing is making a change happen; a vision document promises one change in the reader's
  world *(change; TIM, introductory part)*. Write it as one sentence in the form "We change [reader]
  from [state today] to [state after]" *(audience; TIM ch. 4)* — e.g. "We change agent builders from
  waiting on network round-trips to working on local data at memory speed."
- Fill the three-sentence promise as a test before drafting: "This is for people who believe ___ /
  We focus on people who want ___ / We promise that engaging with this will help you get ___"
  *(positioning; TIM ch. 4)*. If a blank cannot be filled concretely, the document is not ready to be
  written.
- If the change cannot be talked about — if a reader could not retell it to a colleague in one
  sentence — find a different change worth making *(channels; TIM ch. 17)*.

## 2. The opening: headline, subline, first paragraph

Readers scan, they do not study, and while scanning they ask "what does this remind me of?"
*(brand-meaning; TIM ch. 13; KOB ch. 2)*. The opening must answer three questions in this order:
WHAT is it, FOR WHOM, and WHY should I care now.

- **Headline — a concrete frame of reference plus the point of difference.** People learn new
  things by relating them to familiar ones; for a NEW product an abstract, goal-only frame ("data
  freedom", "the future of collaboration") is the most likely failure *(positioning; KOB ch. 1)*.
  Name a category the reader already knows, then what is different within it. Pattern:
  "[Known category] that [difference that matters to the reader]."
- **Subline — the reader and the change.** Who it is for and what changes for them, in their
  words, not yours.
- **First paragraph — problem, why now, what it does.** Three to four sentences. No history of the
  project, no team introduction, no list of technologies.
- Abstraction belongs to strategy papers; a public text needs target, category and point of
  difference *(positioning; KOB ch. 1)*. Visionary language may come later in the document — never
  in place of the frame.
- A known sign, changed just enough: send a signal that looks like something the reader already
  trusts, then change it enough to show it is new and yours *(brand-meaning; TIM ch. 13)*. An RFC or
  design-doc form is such a sign for technical readers.

**The opening test.** Show only headline, subline and first paragraph to someone of the target
type, for a few seconds, then take it away and ask: What is it? Who is it for? Why would you read
on? Brief exposure measures perception; a discussion group invites people to play design expert
and hides the real effect *(brand-meaning; KOB ch. 2)*. If two of the three answers are wrong or
vague, rewrite the opening before touching anything else. If no reader is available, run the test
yourself against the three questions and say so in the review.

## 3. The body, in this order

### Problem
- Start from the group, their problem and the change they seek — not from the solution or from
  what makes you clever *(positioning; TIM ch. 6)*.
- Describe the problem the way the reader experiences it. They do not believe what you believe,
  know what you know or want what you want *(audience; TIM ch. 5)*. A problem described in your
  architecture's terms is your problem, not theirs.

### Why now
- Name what has changed so that the old way no longer holds. People without an established pattern
  — a new platform, a new way of working — do not have to be persuaded that their old choices were
  mistakes *(change; TIM ch. 10)*. If such a moment exists for your readers, it is your "why now".
- Every new thing breaks something: say honestly which current practice stops being good enough
  *(change; TIM ch. 10)*. That creates tension — the promise that one can get through to the other
  side. Not fear: if a draft frightens, coerces or manipulates, it is doing it wrong *(change; TIM
  ch. 10)*.

### Vision
- Here, and only here, the document may lift to the level of goals and feelings. Anchor it in the
  frame set in the opening so it reads as where THIS thing leads, not as a slogan *(positioning; KOB
  ch. 1)*.
- Do not climb the ladder further than the category allows. Infrastructure is mostly judged
  functionally; the emotional benefit for a technical reader is usually confidence, control or
  being seen as the one who spotted it early — not delight *(positioning; KOB ch. 1; audience; TIM
  ch. 11)*.

### Use cases — each a concrete situation of a named reader type
For every use case:
- **Who** — a named reader type with a problem, a desire and a narrative (e.g. "the developer
  building an offline-capable field app", "the team lead who has to answer the security
  review") *(audience; TIM ch. 4)*.
- **Before** — what they do today, in concrete steps, and what it costs them.
- **After** — the same situation with the product: what they do, what they no longer do.
- **What they will tell others** — the sentence the reader would say to a colleague or boss. In
  business settings: "What will I tell my boss?" — the answer must be better than "it was the
  cheapest" *(b2b; TIM ch. 19)*.
- Be honest about who is well served today. Complete the sentence "For people who want ___ and
  believe ___, their current choice of ___ is exactly right" *(audience; TIM ch. 8)*. A use case that
  pretends today's tools are stupid loses the very readers who know them best.
- Three use cases that differ in reader type beat ten variations for the same reader. If two use
  cases have the same "after", merge them.

### What it is NOT
- Non-goals and limits, stated plainly. A new product that claims a category without having that
  category's basic properties is not believed — readers know *(positioning; KOB ch. 1, points of
  parity)*. Say which properties of the familiar category you share and which you deliberately do
  not have yet.
- Who should not use it (from section 1).
- If there is a well-known alternative: do not define yourself as "[pioneer], but cheaper/lighter".
  A me-too position signals that the pioneer holds the best position; differentiate far enough
  that the pioneer cannot simply copy you *(diffusion; KOB ch. 4)*. State the different choice you
  made and why, in one or two sentences, without attacking.

### Open questions — the reason the document exists
- Ask for advice, not feedback. Feedback invites people to prove you wrong; advice asks how it
  could fit their world better and reveals their fears and wants *(audience; TIM ch. 8)*.
- Every question is specific and answerable by one reader from their own experience, and it is
  tied to a decision you will take. Pattern: "We are deciding [X]. In your [situation], would you
  rather [A] or [B], and why?" Never "Thoughts?", "Is this useful?" or "What do you think?".
- Say for each question what you will decide with the answers.
- Three to seven questions. More means the vision is not yet decided enough to be shown.
- When answers come back: generalisations ("nobody", "everyone") usually hide one specific sore
  point — find it *(audience; TIM ch. 8)*. And people are good at knowing their wants, poor at
  inventing new ways to meet them *(audience; TIM ch. 7)*: weigh answers about problems higher than
  feature requests.

### How to give feedback
- One obvious place, named at the end and near the top. Prefer a place the project controls over a
  borrowed platform: on someone else's platform you rent the relationship rather than own it
  *(channels; TIM ch. 17)*. Use platforms to point to the document, not to hold the conversation.
- Make a promise about what happens with the feedback — e.g. "we answer every comment within a
  week and publish a summary of what we changed" — and then keep exactly that promise. Permission
  is earned by a promise kept, and lost by doing more than was agreed *(channels; TIM ch. 17)*.
- Offer a way to hear what happens next (watch, subscribe, mailing list) — the people who would miss
  you if you stopped writing are the asset *(channels; TIM ch. 17)*.

## 4. What makes early adopters respond in a technology market
- The first readers are the curious few at the left of the adoption curve. They forgive missteps
  from those who innovate with them; you cannot be perfect for them, the best you can be is
  interesting *(audience; TIM ch. 5, ch. 14)*. Write for them now, not for the cautious majority, who
  want something that works and is safe — a pattern match, not an interruption *(diffusion; TIM
  ch. 19)*. A vision document that tries to reassure the majority bores the people who would answer.
- Give them a reason to pass it on. The two bridge questions: What will I tell my friends? Why will I
  tell them? People share because it benefits them — their taste, their standing as someone who
  spots important ideas — not because you ask *(diffusion; TIM ch. 19; channels; TIM ch. 17)*.
- If the product works better when colleagues also use it, say so in a use case: that is the
  strongest reason to spread it *(diffusion; TIM ch. 8, ch. 19)*.
- Aim at one community you can actually reach and serve — a local chasm, not the global one
  *(diffusion; TIM ch. 19)*. Name it, publish where it gathers, and stay.
- In technology markets, building recognition before the product exists to define the category is
  a legitimate move — thought leadership ahead of availability *(b2b; KOB ch. 11)*. It only works if
  the category is named concretely (section 2).

## 5. Credibility without a product
- Never invent traction, users, benchmarks, partners or quotes. No "trusted by", no waiting-list
  numbers you do not have, no performance figures you have not measured. One invented number
  discredits every true sentence around it.
- Concrete claims are their own reason to believe; abstract claims need proof *(positioning; KOB
  ch. 1)*. Without a product, the proof you have is concreteness: the design decisions and their
  trade-offs, a published draft specification or interface, a prototype or code someone can run,
  what you will deliberately not do, and who you are and what you have built before.
- Claims must be true and backed up in everything you do afterwards; borrowed or decorative
  differentiation does not hold over time in a market where everything can be inspected
  *(positioning; TIM ch. 5)*. In an open technical community, everything can be inspected.
- Banned words, because every competitor uses them: "best", "world-class", "highest performance",
  "next-generation", "revolutionary". Test every value statement: could a competitor say the same
  sentence? Then it is not a value proposition *(b2b; KOB ch. 9)*.
- What buyers cannot test in advance — above all, how you will respond when something goes wrong —
  is what a reputation conveys *(b2b; KOB ch. 9)*. Before a product, the call for feedback itself is
  the first proof: the way you answer is the evidence.

## 6. Review checklist (run it over every draft)
Go through it in order; the first failing item is usually the most important fix.

1. Opening test: from headline, subline and first paragraph alone, can a reader of the target type
   say what it is, for whom, and why read on? *(brand-meaning, positioning)*
2. Is the headline's frame a category the reader already knows, plus a difference — not an
   abstract goal? *(positioning)*
3. Is there one reader group, described by worldview, and is it said who it is NOT for?
   *(audience, positioning)*
4. Is there exactly one promised change, statable as "from ___ to ___"? *(change, audience)*
5. Does the problem section use the reader's words, not the architecture's? *(audience)*
6. Is "why now" a real change in the reader's world, and is the tension honest rather than
   fear-driven? *(change)*
7. Does every use case have a named reader type, a concrete before and after, and the sentence they
   would tell a colleague or boss? *(audience, b2b)*
8. Does "what it is not" name limits and parity gaps honestly, and avoid a me-too definition
   against a known alternative? *(positioning, diffusion)*
9. Are the open questions specific, answerable, tied to decisions, and phrased as requests for
   advice? No "thoughts?" *(audience)*
10. Is there one obvious, owned place for feedback, with a promise about what happens to it?
    *(channels)*
11. Is every claim true and checkable — no invented traction, no banned superlatives, no sentence a
    competitor could sign? *(positioning, b2b)*
12. Does the draft break any decision the project has already taken (naming, tone, terms,
    positioning)? That is the safest finding of all.
13. Length: can anything be cut without the reader losing what it does, for whom, and why now? Cut
    it.

## 7. Which lenses to apply to a draft
For a review, write the dossier (the draft verbatim with line numbers, the project's prior
decisions, the goal, what the answer is for) and apply these lenses — individually by you, or as
independent sub-agents as SKILL.md describes:
- **positioning** — frame of reference, point of difference, the headline.
- **audience** — who it is for and not for, the use-case reader types, the questions.
- **diffusion** — early adopters, the reason to pass it on, the first community.
- **b2b** — the value proposition test, user vs buyer, "what will I tell my boss", technology-market
  specifics.
- **brand-meaning** — what the name and the promise will come to stand for, and whether the promise
  can be kept over time.
- **channels** — where to publish, where to hold the conversation, the feedback promise.
Add **change** when the promised change or the "why now" is the weak point, and **measurement** when
the owner wants to know how to judge the response to the document.

When the request is to WRITE the document: establish the facts, fill section 1 (if a blank cannot
be filled from the material, stop and ask the user exactly one question about it, with your best
guess), draft in the order of section 3, then run the checklist yourself before handing it over. The
answer is the draft, followed by what you would leave out and what you will not commit to (SKILL.md,
"What you return").

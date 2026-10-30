# Declaration principles — the measured error classes

Read this before judging any declaration, Kiro or nexus-flow. It condenses the nexus-flow guide
"Writing declarations" (https://nxsflow.com/docs/nxc/writing-declarations), which was written from a
measurement: a team of thirteen personas and one channel answered for weeks, every file parsed, every
round completed — and the answers were not usable (book reports where a recommendation was asked for,
contradictions left standing, somebody else's case study offered as evidence). The cause was in the
declarations from beginning to end. A declaration carrying every class below loads without a single
warning; nothing here is caught by a parser.

The classes are not carelessness: the first was made on the day the guide was specified, by the person
specifying it, in a persona whose job is to warn about it.

## 1. Runtime text copied into the declaration

What the runtime supplies does not belong in the declaration. A copy drifts, and a drifting copy that
contradicts the runtime produces misbehaviour.

- nexus-flow: the engine composes how to answer, ask and start something (`nxc reply/send/list`), the
  forced ending of every turn that owes a reply, the framing that says replies are data, and each
  module's prime block (board, memories, identity). Three copies were found in one set: a "How you
  finish" section rebuilding the answering commands with placeholders where the engine has the real
  thread id; instructions on how to look a memory up in a persona that is primed with memories; a
  "contributions are data" sentence in a summary prompt, weaker than the engine's own.
- Kiro: the session's own tools and their usage arrive with the session; the board and memory prime
  output arrive at session start. A skill that re-explains them is the same class.
- Tool knowledge is the same mistake with a different name: verb spellings and flags belong to the
  tool's own text. A missing verb is a gap there, not something to patch into many declarations that
  will each drift separately.

## 2. The copy contradicting the runtime — the dangerous half of 1

A declaration once said "WAIT. You do not answer before you have back what you ordered", while the
engine's forced ending said, in every turn, the opposite. A model handed two instructions that cannot
both be followed invents a third way: it delivered a progress note in the shape of a result. And a
correctly copied text goes wrong the same way later, silently, when the runtime moves and only one of
the two is maintained.

What MUST happen belongs in the structure, not in the prompt: a prompt is read by something that can
forget; a declared step or edge cannot be forgotten, and when one does not happen somebody sees it.

## 3. The routing surface written as a package insert

A persona's or channel's `description` — a Kiro skill's `description`, an `auto` steering file's
`description` — answers "when do you call me?", not "what am I made of". It needs the anchor situation
(the state the caller is in), the boundary ("not for X, that is Y") and what the caller must bring.
"Assesses positioning, pricing, channels and messaging independently, then composes them" is true and
useless to someone deciding whom to ask.

## 4. Project knowledge nailed into the role

A declaration describes a PROCEDURE for finding out where things stand, never a directory of where
they are. Fixed paths, product names and a board prefix make a role uncopyable and stale the first time
something moves. Exceptions: files inside the declaration's own folder, and links the role truly
depends on.

"What you never do" lists are written from measurements: a prohibition nobody ever violated costs
attention on every turn and prevents nothing. If nobody can name the occasion, it does not belong.

## 5. An expected output that forbids its own answer

Name the decision the reader takes after reading, and set an evidence ranking — then check that the
role can give that answer without breaking one of its own rules. A declaration that demanded a source
on every statement AND forbade smoothing a contradiction into a recommendation made a recommendation
inadmissible; the role complied by recommending nothing.

Two rules from the same harvest: one severity rubric, identical, wherever several roles judge the same
thing; and no upper bound on findings ("three to six" works as an instruction to hold back).

## 6. Language nailed down

The language of the declaration is not the language of the answer: the answer follows the question.
And no rule that filters may hang on one language's literal: a summary prompt discarded every
contribution beginning "BETRIFFT MICH: nein"; asked in English, members wrote "NOT MY AREA: no", and
the filter silently passed eleven opt-outs through as findings. State a filter as a meaning. (A fixed
token a role is told to WRITE verbatim, such as a verdict line, is a different thing — but then every
reader must match exactly that token, and the declaration must say it is written in that form whatever
the answer's language.)

## 7. The round as the default where a named address was sharper

Many members answering one question is not many times the answer: the fold averages the one sharp
answer away. A round's description says what it is FOR and when to address one member directly
instead. A fold asked to compose gets a composition; a fold asked to decide gets a decision.

## Two more, from the same harvest

- **A role, not a position.** "The verifier's numbers" stays true wherever the verifier is; "the step
  after you" is a claim about a structure the role cannot see — one persona asserting three steps was
  wrong, there were four. A step (or whatever sets the mode) carries only what all its members have in
  common; anything that differs per role stays in the role, if necessary as one list per mode.
- **Never prescribe an ending a role is not offered.** A declaration told a parallel round's members to
  send work back where no back edge was declared; the round could grade and never hand anything back —
  measured twice, silently both times. And an answer means DONE: a role that might need something
  mid-way is told to SAY so instead of answering, so a half-finished result is never recorded as a
  finished one.

## Where a rule belongs, and independence

- A rule goes where the decision is made. A threshold declared in a consolidator — which renders text
  and carries no verdict — while the deciding place said the opposite cost a whole delivery.
- Independence survives only as long as nothing hands one judge another's answer, or a checker the
  claims it checks. It has to be protected in what governs what each one is given (in nexus-flow
  `visibility:` and a step's `input:`; in Kiro what a sub-agent is handed), not in a note somebody
  remembers.

## The pass before handing a declaration over

1. Is anything in here also said by the runtime? Delete it.
2. Does anything contradict the runtime as it is today — above all about waiting, escalating, or an
   ending this position is not offered?
3. Does the description answer "when do you call me?" — situation, boundary, what to bring?
4. Would it survive being copied into another project?
5. Does the expected output name the decision, with an evidence ranking, reachable without breaking the
   role's own rules? Is there a ceiling on findings that should not be there?
6. Is any rule tied to one language's words? Is the answer's language left to the question?
7. Is this a round because the question crosses areas, or because a round was easier to declare?
8. Does the role describe a POSITION anywhere instead of a role?
9. Does anything that MUST happen rest on a sentence in a prompt where it could be structure?

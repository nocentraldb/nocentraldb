# Kiro declaration format — what the loader requires

Read this whenever a change touches a Kiro skill or steering file. A format violation is checkable:
quote the line and say what the loader does with it. A file that is refused is the good case; the
expensive ones load and then do nothing.

Source of truth for the format: the Kiro documentation (kiro.dev/docs/skills, kiro.dev/docs/steering)
and the Agent Skills specification it points to (agentskills.io/specification). Where a finding depends
on the current Kiro behaviour rather than on the rules below, check the documentation — it moves.

## Skills — `.kiro/skills/<name>/SKILL.md`

- One skill per folder; `SKILL.md` is required. Optional sub-folders: `references/` (documents),
  `scripts/` (executable code), `assets/` (templates, files used in output).
- The YAML frontmatter is the first thing in the file, between `---` lines.
- `name` — required. Identical to the folder name. Lowercase letters, digits and hyphens only; no
  leading, trailing or doubled hyphen; at most 64 characters. A workspace skill overrides a global
  one (`~/.kiro/skills/`) of the same name — check whether a new name shadows one deliberately.
- `description` — required, at most 1024 characters (count them). It is the ROUTING SURFACE: at
  session start Kiro loads only `name` and `description` of every skill, and loads the body only when a
  request matches the description (or the user types `/<name>`). So it is judged as a signpost:
  - the anchor situation ("Use when …"), in words a caller recognises as their own;
  - the boundary ("Not for X — use the `y` skill"), which stops the wrong call;
  - what the caller must bring, where the result is worthless without it.
  A list of what the skill is made of, or how it works inside, is a package insert, not a signpost
  (a Medium where it makes the skill unfindable or wrongly found).
- Optional: `license`, `compatibility` (environment requirements), `metadata` (a string-to-string map).
  `allowed-tools` is part of the Agent Skills specification but Kiro does not document honouring it —
  a rule that depends on it rests on nothing Kiro promises.
- An `nxs:` block is nexus-flow-only: Kiro ignores it. Anything in it that must still hold
  (`expected_output`, independence, addressability) has to be in the body or it is lost.
- References load only when the body tells the agent to read them. A reference the body never names
  is dead weight; a body that names a reference without saying WHEN to read it leaves the choice to
  chance; a relative path that does not exist is a defect. Paths are relative to the skill folder.
- Keep the body focused — about 500 lines at most; self-contained parts belong in `references/`.
- Custom agents (`.kiro/agents/*.json`) do not load skills unless their `resources` include
  `skill://…` entries — a skill meant for such an agent is unreachable otherwise.
- Text after `/<name>` reaches the skill as context; substitution into `$ARGUMENTS` placeholders is
  CLI-only, so a skill that relies on it does not work in the IDE.

## Steering — `.kiro/steering/*.md`

- The YAML frontmatter is the very first content — no blank line before the opening `---`.
- `inclusion` decides when the file is loaded:
  - `always` (the default when absent) — every interaction. It costs attention in every session, so
    its "never do" lists must come from measured incidents; one nobody ever violated prevents nothing.
  - `fileMatch` — needs `fileMatchPattern`, a glob string or an array of globs; loaded when the work
    touches matching files. A pattern that matches nothing in the repository makes the file dead.
  - `manual` — pulled in by the user as `#<file-name>` in chat (also offered as a slash command).
    Nothing mandatory may rest on a manual file.
  - `auto` — needs `name` and `description`; loaded when a request matches the description, like a
    skill, so the description is judged as a signpost exactly like a skill's.
- File references: `#[[file:<relative path>]]` includes another file, resolved from the workspace root
  for workspace steering (from `~/.kiro/steering/` for global steering). Line and range forms and
  `#[[folder:…]]` are not available on every surface. A reference that does not resolve leaves a
  visible marker — check every path.
- `AGENTS.md` files are always included and take no inclusion mode; their paths resolve from their own
  folder.
- Workspace steering wins over global steering on conflict — a workspace file contradicting a global
  one silently replaces it.
- Older CLI versions load only `always` files: a `fileMatch`, `manual` or `auto` file whose rule MUST
  apply is not loaded there. Where the project's agents run on such a surface, that is a finding.
- Custom agents do not load steering unless their `resources` include the steering files.

## The project's own generated files

Files the project marks as generated or managed (for example a block between "managed, do not edit"
markers in `AGENTS.md`, or a memory projection regenerated from `nxm`) are changed at their source,
never by hand. A change that edits them directly will be overwritten — say where the change belongs.

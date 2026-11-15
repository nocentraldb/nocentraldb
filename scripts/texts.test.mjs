// Tests of scripts/texts.mjs (NOC-329): the repository's texts pass, and
// each check fails on its mutant.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  CLI_STRAYS,
  checkCode,
  checkEntries,
  cliUse,
  checkLayout,
  constName,
  diffTexts,
  enumVariants,
  formatReview,
  generate,
  generateCli,
  lastReleaseTag,
  noticeVariants,
  PATHS,
  placeholders,
  review,
} from "./texts.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(ROOT, path), "utf8");

const ENTRY = {
  id: "notice.memberAdded",
  kind: "notice",
  code: "memberAdded",
  text: "The new member reads the whole history of this space.",
  when: "Members::add returns it.",
};
const VARIANTS = ["MemberAdded"];
const file = (...texts) => ({ texts });

test("the repository's texts pass the gate", () => {
  const run = spawnSync(
    process.execPath,
    [join(ROOT, "scripts", "texts.mjs"), "--check"],
    {
      encoding: "utf8",
    },
  );
  assert.equal(run.status, 0, run.stderr);
});

test("the committed module is the one the texts generate, file by file", () => {
  const texts = JSON.parse(read(PATHS.texts)).texts;
  const generated = generate(texts);
  assert.deepEqual(Object.keys(generated).sort(), [
    "error.rs",
    "mod.rs",
    "notice.rs",
    "part.rs",
  ]);
  for (const [name, content] of Object.entries(generated)) {
    assert.equal(read(`${PATHS.module}/${name}`), content, name);
  }
});

test("every NoticeCode of the SDK has its notice, and each notice its code", () => {
  const variants = noticeVariants(read(PATHS.noticeCodes));
  assert.ok(
    variants.includes("MemberAdded") && variants.length >= 11,
    variants.join(),
  );
  const notices = JSON.parse(read(PATHS.texts)).texts.filter(
    (t) => t.kind === "notice",
  );
  assert.deepEqual(
    notices.map((t) => t.code[0].toUpperCase() + t.code.slice(1)).sort(),
    [...variants].sort(),
  );
});

test("a well-formed entry passes", () => {
  assert.deepEqual(checkEntries(file(ENTRY), VARIANTS), []);
});

test("each broken entry fails", () => {
  const mutants = [
    ["no texts array", { texts: {} }],
    ["a missing when", file({ ...ENTRY, when: undefined })],
    ["an empty when", file({ ...ENTRY, when: "  " })],
    ["a missing text", file({ ...ENTRY, text: undefined })],
    ["an unknown field", file({ ...ENTRY, comment: "x" })],
    [
      "a since field, which git history replaces",
      file({ ...ENTRY, since: "unreleased" }),
    ],
    [
      "an unknown kind",
      file({ ...ENTRY, kind: "hint", id: "hint.memberAdded" }),
    ],
    ["a code that is not camelCase", file({ ...ENTRY, code: "member_added" })],
    [
      "an id that does not name kind and code",
      file({ ...ENTRY, id: "notice.added" }),
    ],
    [
      "a code NoticeCode does not have",
      file({ ...ENTRY, code: "memberJoined", id: "notice.memberJoined" }),
    ],
    ["an id used twice", file(ENTRY, ENTRY)],
    [
      "a NoticeCode without its notice",
      { variants: ["MemberAdded", "RoleSet"], file: file(ENTRY) },
    ],
  ];
  for (const [name, mutant] of mutants) {
    const problems = mutant.variants
      ? checkEntries(mutant.file, mutant.variants)
      : checkEntries(mutant, VARIANTS);
    assert.ok(problems.length > 0, `${name} passed`);
  }
});

const MODULE = "src/texts";
const USE = [
  "src/members/notice.rs",
  "text: crate::texts::notice::MEMBER_ADDED,",
];

test("code that uses the texts passes", () => {
  assert.deepEqual(
    checkCode([USE, [`${MODULE}/mod.rs`, "x"]], MODULE, [ENTRY]),
    [],
  );
});

test("a notice text written in code fails, on one line or across lines", () => {
  for (const source of [
    'text: "The new member reads the whole history of this space.",',
    'Notice {\n    text:\n        "written here",\n}',
  ]) {
    const problems = checkCode([USE, ["src/members/mod.rs", source]], MODULE, [
      ENTRY,
    ]);
    assert.equal(problems.length, 1, source);
    assert.match(
      problems[0],
      /^src\/members\/mod\.rs:\d+: a notice text written in code/,
    );
  }
});

test("a text no code uses fails, and the generated module does not count as a use", () => {
  const unused = checkCode(
    [
      [
        `${MODULE}/notice.rs`,
        "pub(crate) const MEMBER_ADDED: &str = notice::MEMBER_ADDED;",
      ],
    ],
    MODULE,
    [ENTRY],
  );
  assert.deepEqual(unused, [
    "notice.memberAdded: no code uses texts::notice::MEMBER_ADDED",
  ]);
});

test("the module holds each text as a Rust literal, with when it shows", () => {
  const tricky = { ...ENTRY, text: 'A "quoted" \\ text\twith é.' };
  const files = generate([tricky]);
  assert.deepEqual(Object.keys(files).sort(), ["mod.rs", "notice.rs"]);
  assert.ok(files["mod.rs"].includes("pub(crate) mod notice;"));
  const module = files["notice.rs"];
  assert.match(module, /^\/\/ Generated by scripts\/texts\.mjs/);
  assert.ok(
    module.includes(String.raw`"A \"quoted\" \\ text\u{9}with é."`),
    module,
  );
  assert.ok(module.includes("/// `memberAdded`: Members::add returns it."));
  assert.ok(module.includes("pub(crate) const MEMBER_ADDED: &str ="));
  assert.equal(constName("accountKeyRenewed"), "ACCOUNT_KEY_RENEWED");
});

test("every notice text RFC 0009 quotes is a copy of texts.json's", () => {
  const rfc = read("docs/rfc/0009-sdk-api.md");
  const texts = new Map(
    JSON.parse(read(PATHS.texts)).texts.map((t) => [t.code, t.text]),
  );
  const quotes = [
    // `{ code: "memberAdded", text: "…" }` in the examples.
    ...[...rfc.matchAll(/\{ code: "(\w+)", text: "([^"]+)" \}/g)].map((m) => [
      m[1],
      m[2],
    ]),
    // `| \`DeviceAdded\` | … | "…" |` in the table of the account notices.
    ...[...rfc.matchAll(/^\| `([A-Z]\w+)` \| [^|]+ \| "([^"]+)" \|$/gm)].map(
      (m) => [m[1][0].toLowerCase() + m[1].slice(1), m[2]],
    ),
    // `| \`erasurePending\` | … with the text "…" (…) |` in the names table.
    ...[...rfc.matchAll(/^\| `(\w+)` \| [^|]*with the text "([^"]+)"/gm)].map(
      (m) => [m[1], m[2]],
    ),
  ];
  assert.ok(quotes.length >= 11, `only ${quotes.length} quotes found`);
  for (const [code, quoted] of quotes) {
    assert.equal(quoted, texts.get(code), `RFC 0009 quotes ${code} otherwise`);
  }
});

test("the file is sorted by id and written one field per line", () => {
  const second = { ...ENTRY, id: "notice.roleSet", code: "roleSet" };
  const sorted = file(ENTRY, second);
  assert.deepEqual(
    checkLayout(`${JSON.stringify(sorted, null, 2)}\n`, sorted),
    [],
  );
  const unsorted = file(second, ENTRY);
  assert.equal(
    checkLayout(`${JSON.stringify(unsorted, null, 2)}\n`, unsorted).length,
    1,
  );
  for (const raw of [
    JSON.stringify(sorted),
    `${JSON.stringify(sorted, null, 4)}\n`,
    JSON.stringify(sorted, null, 2),
  ]) {
    assert.equal(checkLayout(raw, sorted).length, 1, raw.slice(0, 40));
  }
});

const ERROR = {
  id: "error.noSuchSpace",
  kind: "error",
  code: "notFound",
  text: "the copy holds no space {space}",
  when: "A call that names a space this copy does not hold.",
};
const PART = {
  id: "part.kindText",
  kind: "part",
  text: "a text",
  when: "What a read asked for.",
};
const ERRORS = ["NotFound", "StorageFull", "StorageFailed"];

test("every ErrorCode an error names exists in error.rs", () => {
  const variants = enumVariants(read(PATHS.errorCodes), "ErrorCode");
  assert.ok(variants.includes("InvalidValue") && variants.includes("NotFound"));
  const errors = JSON.parse(read(PATHS.texts)).texts.filter(
    (t) => t.kind === "error",
  );
  assert.ok(errors.length >= 80, `${errors.length} errors`);
});

test("well-formed errors and parts pass, with one code or several", () => {
  const several = {
    ...ERROR,
    id: "error.viewFailed",
    code: "storageFull | storageFailed",
  };
  assert.deepEqual(
    checkEntries(file(ENTRY, ERROR, several, PART), VARIANTS, ERRORS),
    [],
  );
});

test("each broken error or part fails", () => {
  const mutants = [
    ["an error without a code", { ...ERROR, code: undefined }],
    ["an error code ErrorCode lacks", { ...ERROR, code: "gone" }],
    [
      "one of several codes ErrorCode lacks",
      { ...ERROR, code: "notFound | gone" },
    ],
    [
      "several codes not split by ' | '",
      { ...ERROR, code: "notFound|storageFull" },
    ],
    ["a part with a code", { ...PART, code: "notFound" }],
    ["an id of another kind", { ...ERROR, id: "part.noSuchSpace" }],
    [
      "an id whose name is not camelCase",
      { ...ERROR, id: "error.no-such-space" },
    ],
    ["a brace that is no placeholder", { ...ERROR, text: "the set {a, b}" }],
    ["an unclosed brace", { ...ERROR, text: "the copy holds no space {space" }],
    ["a notice with a placeholder", { ...ENTRY, text: "added {member}" }],
  ];
  for (const [name, mutant] of mutants) {
    assert.ok(
      checkEntries(file(mutant), VARIANTS, ERRORS).length > 0,
      `${name} passed`,
    );
  }
});

test("a text with placeholders becomes a function that fills them", () => {
  assert.deepEqual(placeholders("{what}: {error:?} and {what}"), [
    ["what", false],
    ["error", true],
  ]);
  const files = generate([
    { ...ERROR, text: "the lock is refused: {why:?} at {space}" },
    PART,
  ]);
  const module = files["error.rs"];
  assert.ok(
    module.includes(
      "pub(crate) fn no_such_space(why: impl std::fmt::Debug, space: impl std::fmt::Display) -> String {",
    ),
    module,
  );
  assert.ok(
    module.includes('format!("the lock is refused: {why:?} at {space}")'),
  );
  assert.ok(files["mod.rs"].includes("pub(crate) mod part;"));
  assert.ok(files["part.rs"].includes("pub(crate) const KIND_TEXT: &str ="));
  assert.ok(module.includes("/// `noSuchSpace` (notFound): A call"));
});

test("an error text written in code fails, in each of its forms", () => {
  const strays = [
    'Error::new(ErrorCode::NotFound, "the copy holds no space")',
    'Error::new(\n    ErrorCode::NotFound,\n    format!("the copy holds no space {id}"),\n)',
    'Error::new(code, concat!("not ", stringify!($name)))',
    'Error::new(ErrorCode::of(x), "written here")',
    'return Err("its version is unknown");',
    'invalid(\n    "the options name no place",\n)',
    'failed("IndexedDB holds no wrapping key")',
    'io("the key file cannot be read", &error)',
    'refused("IndexedDB cannot be read", &error)',
    'holds(resolved, me, Role::Admin, "adds members")',
    'crypto("the key record cannot be sealed", &error)',
  ];
  for (const source of strays) {
    const problems = checkCode([USE, ["src/x.rs", source]], MODULE, [ENTRY]);
    assert.equal(problems.length, 1, source);
  }
});

test("errors built from crate::texts pass, and an unused function fails", () => {
  const source = [
    "Error::new(ErrorCode::NotFound, crate::texts::error::no_such_space(id))",
    "Error::new(code, message)",
    "Err(crate::texts::part::KIND_TEXT)",
    "io(crate::texts::part::KIND_TEXT, &error)",
    'println!("not an error: {x}")',
  ].join("\n");
  assert.deepEqual(
    checkCode([USE, ["src/x.rs", source]], MODULE, [ENTRY, ERROR, PART]),
    [],
  );
  assert.deepEqual(checkCode([USE], MODULE, [ERROR]), [
    "error.noSuchSpace: no code uses texts::error::no_such_space(",
  ]);
});

test("the last release tag is the newest beta or stable one, by number", () => {
  assert.equal(lastReleaseTag([]), null);
  assert.equal(lastReleaseTag(["v0.1.0-alpha.56", "v0.2.0-alpha.1"]), null);
  assert.equal(
    lastReleaseTag(["v0.3.1-beta.2", "v0.3.1-beta.10", "v0.3.1-alpha.99"]),
    "v0.3.1-beta.10",
  );
  assert.equal(
    lastReleaseTag(["v0.3.1-beta.10", "v0.3.1", "v0.3.0"]),
    "v0.3.1",
  );
  assert.equal(
    lastReleaseTag(["v0.10.0-beta.1", "v0.9.9", "v1.0.0-beta.1x"]),
    "v0.10.0-beta.1",
  );
});

test("the review lists new, changed and removed texts with what changed", () => {
  const removed = { ...ENTRY, id: "notice.roleSet", code: "roleSet" };
  const before = [ENTRY, removed, PART];
  const after = [
    { ...ENTRY, text: "The new member reads all of this space." },
    { ...PART, when: "What a read asked for, newly worded." },
    ERROR,
  ];
  const diff = diffTexts(before, after);
  assert.deepEqual(
    diff.added.map((t) => t.id),
    ["error.noSuchSpace"],
  );
  assert.deepEqual(
    diff.changed.map((c) => c.after.id),
    ["notice.memberAdded", "part.kindText"],
  );
  assert.deepEqual(
    diff.removed.map((t) => t.id),
    ["notice.roleSet"],
  );
  const text = formatReview(diff, "v0.3.1-beta.2", 3);
  assert.match(text, /^# Texts new or changed since v0\.3\.1-beta\.2\n/);
  assert.ok(text.includes("1 new, 2 changed, 1 removed, of 3 texts"));
  assert.ok(
    text.includes(
      '- text was: "The new member reads the whole history of this space."',
    ),
  );
  assert.ok(
    text.includes('- text now: "The new member reads all of this space."'),
  );
  assert.ok(text.includes("- when was: What a read asked for."));
  assert.ok(text.includes('- text (unchanged): "a text"'));
  assert.ok(text.includes("## Removed\n\n### notice.roleSet"));
  assert.ok(text.includes("- error · notFound"));
  assert.deepEqual(diffTexts([ENTRY], [ENTRY]), {
    added: [],
    changed: [],
    removed: [],
  });
  assert.match(
    formatReview(diffTexts([], [ENTRY]), null, 1),
    /^# Texts: no beta or stable release yet/,
  );
});

test("pnpm texts compares the file at the last release tag, found in git", () => {
  const dir = mkdtempSync(join(tmpdir(), "texts-review-"));
  try {
    const git = (...args) => {
      const run = spawnSync("git", ["-C", dir, ...args], { encoding: "utf8" });
      assert.equal(run.status, 0, run.stderr);
    };
    const put = (texts) => {
      mkdirSync(join(dir, "texts"), { recursive: true });
      writeFileSync(
        join(dir, "texts", "texts.json"),
        `${JSON.stringify({ texts }, null, 2)}\n`,
      );
      git("add", "-A");
      git(
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "commit",
        "-q",
        "-m",
        "texts",
      );
    };
    git("init", "-q");
    writeFileSync(join(dir, "README"), "x");
    git("add", "-A");
    git(
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@t",
      "commit",
      "-q",
      "-m",
      "before texts",
    );
    git("tag", "v0.1.0");
    assert.throws(() => review(dir), /ENOENT/);
    put([ENTRY]);
    git("tag", "v0.2.0-beta.1");
    put([{ ...ENTRY, text: "Changed after the beta." }, PART]);
    git("tag", "v0.2.0-alpha.3");
    const text = review(dir);
    assert.match(text, /^# Texts new or changed since v0\.2\.0-beta\.1$/m);
    assert.ok(text.includes("1 new, 1 changed, 0 removed, of 2 texts"), text);
    assert.ok(text.includes('- text now: "Changed after the beta."'));
    // A tag from before texts.json: every text is new since it.
    assert.ok(review(dir, "v0.1.0").includes("2 new, 0 changed, 0 removed"));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// The command line's texts (NOC-329, T5): kind `cli`, with no code, in
// their own module, crates/ncdb/src/texts.rs.
const CLI = {
  id: "cli.unknownOption",
  kind: "cli",
  text: "unknown option: {option}",
  when: "ncdb model build given an unknown option.",
};
const CLI_CONST = {
  id: "cli.noCommand",
  kind: "cli",
  text: "no command",
  when: "ncdb run with no arguments.",
};
const CLI_MODULE = "crates/ncdb/src/texts.rs";
const cliCheck = (files, texts = [CLI, CLI_CONST]) =>
  checkCode(files, CLI_MODULE, texts, { strays: CLI_STRAYS, use: cliUse });
const CLI_USES = [
  "crates/ncdb/src/args.rs",
  "return Err(texts::NO_COMMAND.into());\n.map_err(texts::unknown_option)?;",
];

test("a cli text passes without a code, and fails with one", () => {
  assert.deepEqual(checkEntries(file(CLI), []), []);
  assert.ok(checkEntries(file({ ...CLI, code: "unknownOption" }), []).length);
  assert.ok(
    checkEntries(file({ ...CLI, id: "error.unknownOption" }), []).length,
  );
});

test("the cli texts generate the command line's module, not the SDK's", () => {
  const module = generateCli([ENTRY, CLI, CLI_CONST]);
  assert.match(module, /^\/\/ Generated by scripts\/texts\.mjs/);
  assert.match(
    module,
    /pub\(crate\) fn unknown_option\(option: impl std::fmt::Display\) -> String \{\n {4}format!\("unknown option: \{option\}"\)\n\}/,
  );
  assert.match(
    module,
    /pub\(crate\) const NO_COMMAND: &str =\n {4}"no command";/,
  );
  assert.doesNotMatch(module, /MEMBER_ADDED/);
  assert.ok(
    !Object.values(generate([ENTRY, CLI]))
      .join("")
      .includes("unknown"),
  );
});

test("command-line code that uses its texts passes, a function passed by name too", () => {
  assert.deepEqual(cliCheck([CLI_USES, [CLI_MODULE, "x"]]), []);
});

test("a text written in the command line's code fails, in each of its forms", () => {
  for (const [source, what] of [
    ['return Err("no such file here".into());', "a text"],
    ['format!("{lock}: up to date (model version {v})")', "a text"],
    ['pub const USAGE: &str = "usage";', "a text constant"],
    [
      'const S: &str = r#"\nfail(`cannot be run: ${e}`);\n"#;',
      "a message of the schema script",
    ],
    [
      'const S: &str = r#"\nfail("exports no model");\n"#;',
      "a message of the schema script",
    ],
  ]) {
    const problems = cliCheck([CLI_USES, ["crates/ncdb/src/x.rs", source]]);
    assert.ok(
      problems.some(
        (p) =>
          p.startsWith("crates/ncdb/src/x.rs:") &&
          p.includes(`${what} written in code`),
      ),
      `${source}: ${problems}`,
    );
  }
});

test("the report's notation, comments and machine words are not texts", () => {
  const source = [
    '        (0, _) => "no fields".into(), // notation',
    '    // a comment that says "two words"',
    '    ["model", "build", rest @ ..] => (false, rest),',
    '    let _ = writeln!(out, "  + collection {name} ({})", summary(c));',
    '    format!("{lock_name}: {message}\\n")',
  ].join("\n");
  assert.deepEqual(
    cliCheck([CLI_USES, ["crates/ncdb/src/report.rs", source]]),
    [],
  );
});

test("a cli text no code uses fails, and the generated module does not count", () => {
  assert.deepEqual(
    cliCheck([[CLI_MODULE, "texts::NO_COMMAND texts::unknown_option"]]),
    [
      "cli.unknownOption: no code uses /\\btexts::unknown_option\\b/",
      "cli.noCommand: no code uses texts::NO_COMMAND",
    ],
  );
});

test("the command line's committed module is the one the texts generate", () => {
  const texts = JSON.parse(read(PATHS.texts)).texts;
  assert.equal(read(PATHS.cliModule), generateCli(texts));
});

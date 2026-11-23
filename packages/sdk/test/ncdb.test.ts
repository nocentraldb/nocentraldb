// `npx ncdb model build` and `npx ncdb model check` behave as the native
// `ncdb` does (NOC-415): on the same project, both say the same, exit with
// the same code and write the same lock.
import { beforeAll, describe, expect, test } from "vitest";
import { buildBoth, type Files, type Run, runIn, schema } from "./ncdbRuns.ts";

const V1 = schema(`
export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string().describe("One line; what has to be done."),
      status: nc.oneOf(["open", "in_progress", "done"]),
      priority: nc.int(),
      dependsOn: nc.links("tasks"),
    },
    {
      computed: { blocked: nc.reach("dependsOn", { status: ["open", "in_progress"] }) },
      views: { ready: nc.view("tasks", { where: { blocked: false }, sort: ["priority"] }) },
      indexes: { byStatusPriority: nc.index(["status", "priority"], { equal: 1 }) },
    },
  ),
});
`);

const V2 = schema(`
export const model = nc.model({
  people: nc.collection({ name: nc.string().personal({ person: "self" }) }),
  tasks: nc.collection(
    {
      title: nc.string().describe("One line; what has to be done."),
      status: nc.oneOf(["open", "in_progress", "done"]),
      priority: nc.int(),
      tags: nc.set(nc.string()),
      assignee: nc.link("people"),
      body: nc.richText(),
      dependsOn: nc.links("tasks"),
    },
    {
      computed: { blocked: nc.reach("dependsOn", { status: ["open", "in_progress"] }) },
      views: { ready: nc.view("tasks", { where: { blocked: false }, sort: ["priority"] }) },
      indexes: {
        byStatusPriority: nc.index(["status", "priority"], { equal: 1 }),
        byPriorityDesc: nc.index([{ field: "priority", direction: "desc" }]),
      },
    },
  ),
});
`);

/** V1 without a field: a change old clients cannot read. */
const WITHOUT_PRIORITY = V1.replace("priority: nc.int(),", "").replace(
  /nc\.index\(\["status", "priority"\], \{ equal: 1 \}\)/,
  'nc.index(["status"])',
);

let both: { native: string; npm: string };
/** The lock the native build writes from V1, the start of later runs. */
let lockV1: string;

beforeAll(() => {
  both = buildBoth();
  const first = runIn({ native: both.native }, { "model.ts": V1 }, [
    "model",
    "build",
  ]);
  if (first.lock === null) throw new Error(first.stderr);
  lockV1 = first.lock;
}, 600_000);

/** `run` with the random model id of a first build replaced by `<id>`. */
const withoutModelId = (run: Run): Run => {
  if (run.lock === null) return run;
  const id: string = JSON.parse(run.lock).model.id;
  return {
    ...run,
    stdout: run.stdout.replaceAll(id, "<id>"),
    lock: run.lock.replaceAll(id, "<id>"),
  };
};

/** Runs both on the same project and expects the same run from each. */
const same = (files: Files, args: string[], lock?: string): Run => {
  const native = withoutModelId(
    runIn({ native: both.native }, files, args, lock),
  );
  const npm = withoutModelId(runIn({ npm: both.npm }, files, args, lock));
  expect(npm).toEqual(native);
  return npm;
};

describe("npx ncdb says what the native ncdb says", () => {
  test("for --help", () => {
    expect(same({}, ["--help"]).code).toBe(0);
  });

  test("without a command, or with an unknown one", () => {
    expect(same({}, []).code).toBe(2);
    expect(same({}, ["deploy"]).code).toBe(2);
    expect(same({}, ["model", "publish"]).code).toBe(2);
    expect(same({}, ["model", "build", "--model"]).code).toBe(2);
    expect(same({}, ["model", "build", "--force"]).code).toBe(2);
  });

  test("when there is no schema module", () => {
    expect(same({}, ["model", "build"]).code).toBe(1);
  });

  test("when the schema module throws, or exports no model", () => {
    const threw = same({ "model.ts": 'throw new Error("boom");\n' }, [
      "model",
      "build",
    ]);
    expect(threw.code).toBe(1);
    const noModel = same({ "model.ts": "export const x = 1;\n" }, [
      "model",
      "build",
    ]);
    expect(noModel.code).toBe(1);
  });
});

describe("npx ncdb model build writes what the native ncdb writes", () => {
  test("for a first build, up to its random model id", () => {
    const run = same({ "model.ts": V1 }, ["model", "build"]);
    expect(run.code).toBe(0);
    expect(run.lock).not.toBeNull();
  });

  test("for a new model version, rich text included", () => {
    const run = same({ "model.ts": V2, "model.lock.json": lockV1 }, [
      "model",
      "build",
    ]);
    expect(run.code).toBe(0);
    expect(run.lock).not.toBe(lockV1);
  });

  test("for a lock that is up to date", () => {
    const run = same({ "model.ts": V1, "model.lock.json": lockV1 }, [
      "model",
      "build",
    ]);
    expect(run).toMatchObject({ code: 0, lock: lockV1 });
  });

  test("for paths given with --model and --lock", () => {
    const run = same(
      { "schema/app.ts": V2, "out/app.lock.json": lockV1 },
      [
        "model",
        "build",
        "--model",
        "schema/app.ts",
        "--lock",
        "out/app.lock.json",
      ],
      "out/app.lock.json",
    );
    expect(run.code).toBe(0);
  });

  test("for a change old clients cannot read, which it refuses", () => {
    const run = same(
      { "model.ts": WITHOUT_PRIORITY, "model.lock.json": lockV1 },
      ["model", "build"],
    );
    expect(run).toMatchObject({ code: 1, lock: lockV1 });
  });

  test("for a lock edited by hand, which it refuses", () => {
    const edited = lockV1.replace("One line;", "Two lines;");
    const run = same({ "model.ts": V1, "model.lock.json": edited }, [
      "model",
      "build",
    ]);
    expect(run).toMatchObject({ code: 1, lock: edited });
  });
});

describe("npx ncdb model check answers what the native ncdb answers", () => {
  test("for a lock that is up to date", () => {
    const run = same({ "model.ts": V1, "model.lock.json": lockV1 }, [
      "model",
      "check",
    ]);
    expect(run.code).toBe(0);
  });

  test("for a lock that is out of date", () => {
    const run = same({ "model.ts": V2, "model.lock.json": lockV1 }, [
      "model",
      "check",
    ]);
    expect(run).toMatchObject({ code: 1, lock: lockV1 });
  });

  test("when there is no lock", () => {
    const run = same({ "model.ts": V1 }, ["model", "check"]);
    expect(run).toMatchObject({ code: 1, lock: null });
  });
});

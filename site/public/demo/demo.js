// The landing page's live demo (NOC-319): the example task list of
// apps/tasks-ts (NOC-297), running on the visitor's own browser. Its
// database lives on the origin private file system (OPFS) and syncs with
// no relay, so nothing leaves the browser; a second tab of the page shares
// it (NOC-177). The page's loader imports this module after the page has
// rendered; the SDK, its worker and its WebAssembly core load with it.
//
// It declares the same model as apps/tasks-ts/model.ts, whose lock
// scripts/demo-assets.mjs copies next to it, and shows the tasks as that
// app does: each list a subscription to a query of the index `byStatus`
// (RFC 0009, "Queries and indexes", "Subscribe").
import { nc } from "./sdk/index.js";

export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
    },
    {
      indexes: {
        byStatus: nc.index(["status", "title"], { equal: 1 }),
      },
    },
  ),
});

/** How many tasks a list shows. */
const PAGE = 50;

/** The lines of a list, in the page's order, without deleted tasks. */
const rows = (items) =>
  items
    .filter((item) => !item.$deleted)
    .map((item) => ({
      id: item.$id,
      space: item.$space,
      title: item.title || "(untitled)",
      done: item.status === "done",
    }));

/** The space the tasks are added to: the database's first, or a new one. */
const theSpace = async (db) => {
  const [first] = await db.spaces.list();
  return first ? db.spaces.get(first.id) : db.spaces.create();
};

/**
 * Opens the demo's database and shows it in `root`, the page's demo
 * section. `say(state, text)` updates the section's status line.
 */
export const start = async (root, say) => {
  const find = (selector) => {
    const found = root.querySelector(selector);
    if (found === null) throw new Error(`the demo section has no ${selector}`);
    return found;
  };
  say(
    "loading",
    "Loading the demo: opening a database in this browser's private file system (OPFS).",
  );
  const lock = await (
    await fetch(new URL("./model.lock.json", import.meta.url))
  ).json();
  const db = await nc.open({ model, lock, name: "landing-demo" });
  addEventListener("pagehide", () => {
    db.close().catch(() => {});
  });
  const space = await theSpace(db);
  const spaces = new Map([[space.id, Promise.resolve(space)]]);
  const spaceOf = (id) => {
    let found = spaces.get(id);
    if (found === undefined) {
      found = db.spaces.get(id);
      spaces.set(id, found);
    }
    return found;
  };
  const report = (error) => {
    find("[data-demo-error]").textContent = String(error?.message ?? error);
  };

  /** One line: a tick for done, the title, and Delete. */
  const line = (row) => {
    const item = document.createElement("li");
    const label = document.createElement("label");
    const tick = document.createElement("input");
    tick.type = "checkbox";
    tick.checked = row.done;
    tick.addEventListener("change", () => {
      spaceOf(row.space)
        .then((s) =>
          s.update(model.tasks, row.id, {
            status: tick.checked ? "done" : "open",
          }),
        )
        .catch(report);
    });
    const title = document.createElement("span");
    title.className = "title";
    title.textContent = row.title;
    label.append(tick, " ", title);
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "demo__delete";
    remove.textContent = "Delete";
    remove.setAttribute("aria-label", `Delete ${row.title}`);
    remove.addEventListener("click", () => {
      spaceOf(row.space)
        .then((s) => s.delete(model.tasks, row.id))
        .catch(report);
    });
    item.append(label, remove);
    return item;
  };

  for (const status of ["open", "done"]) {
    const list = find(`#demo-${status}`);
    const query = model.tasks.indexes.byStatus.equal(status).take(PAGE);
    db.subscribe(query, (page) => {
      list.replaceChildren(...rows(page.items).map(line));
    });
  }
  const form = find("#demo-add");
  const input = find("#demo-add input[name=title]");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const title = input.value.trim();
    if (title === "") return;
    input.value = "";
    space.create(model.tasks, { title, status: "open" }).catch(report);
  });
  find("[data-demo-app]").hidden = false;
  say(
    "ready",
    "Running in your browser. Your tasks stay here, on this device. Open this page in a second tab, and the two share one database.",
  );
};

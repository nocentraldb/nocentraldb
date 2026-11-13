// The browser task list: one device, its database on OPFS. Each list is a
// subscription to a query of the index `byStatus`: the page shows what the
// subscription gives it, at once and again after every write that touched
// a task (RFC 0009, "Queries and indexes", "Subscribe").
import { type Db, nc, type Space } from "@nocentraldb/sdk";
import { model } from "../model.ts";
import { more, pageSize, type Row, rows, titleOf, toggled } from "./tasks.ts";

/** The element `selector` names; the page holds every one the app uses. */
const element = <T extends HTMLElement>(selector: string): T => {
  const found = document.querySelector<T>(selector);
  if (found === null) throw new Error(`the page has no ${selector}`);
  return found;
};

/** Shows what went wrong; the app goes on. */
const report = (error: unknown) => {
  element("#error").textContent = String(error);
};

/** The space the tasks are added to: the device's first, or a new one. */
const theSpace = async (db: Db): Promise<Space> => {
  const [first] = await db.spaces.list();
  return first ? db.spaces.get(first.id) : db.spaces.create();
};

/** Each task's space, by id, as the device opened it. */
const spaces = new Map<string, Promise<Space>>();
const spaceOf = (db: Db, id: string): Promise<Space> => {
  let space = spaces.get(id);
  if (space === undefined) {
    space = db.spaces.get(id);
    spaces.set(id, space);
  }
  return space;
};

/** One line: a tick for done, the title, and Delete. */
const line = (db: Db, row: Row): HTMLLIElement => {
  const item = document.createElement("li");
  const label = document.createElement("label");
  const tick = document.createElement("input");
  tick.type = "checkbox";
  tick.checked = row.done;
  tick.addEventListener("change", () => {
    spaceOf(db, row.space)
      .then((space) =>
        space.update(model.tasks, row.id, { status: toggled(tick.checked) }),
      )
      .catch(report);
  });
  const title = document.createElement("span");
  title.className = "title";
  title.textContent = row.title;
  label.append(tick, " ", title);
  const remove = document.createElement("button");
  remove.type = "button";
  remove.textContent = "Delete";
  remove.addEventListener("click", () => {
    spaceOf(db, row.space)
      .then((space) => space.delete(model.tasks, row.id))
      .catch(report);
  });
  item.append(label, " ", remove);
  return item;
};

/** Follows the tasks of one status into the list `id`, a page at a time. */
const follow = (db: Db, status: "open" | "done", id: string, step: number) => {
  const list = element<HTMLUListElement>(`#${id}`);
  const button = element<HTMLButtonElement>(`#${id}-more`);
  let limit = step;
  let stop = () => {};
  const start = () => {
    stop();
    const query = model.tasks.indexes.byStatus.equal(status).take(limit);
    stop = db.subscribe(query, (page) => {
      list.replaceChildren(...rows(page.items).map((row) => line(db, row)));
      button.hidden = !page.hasMore;
    });
  };
  button.addEventListener("click", () => {
    limit = more(limit, step);
    start();
  });
  start();
};

const main = async () => {
  const lock: unknown = await (await fetch("/model.lock.json")).json();
  const db = await nc.open({ model, lock, name: "tasks" });
  addEventListener("pagehide", () => {
    db.close().catch(() => {});
  });
  const space = await theSpace(db);
  spaces.set(space.id, Promise.resolve(space));
  const step = pageSize(location.search);
  follow(db, "open", "open", step);
  follow(db, "done", "done", step);
  const form = element<HTMLFormElement>("#add");
  const input = element<HTMLInputElement>("#add input[name=title]");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const title = titleOf(input.value);
    if (title === undefined) return;
    input.value = "";
    space.create(model.tasks, { title, status: "open" }).catch(report);
  });
  document.body.dataset.ready = "true";
};

main().catch(report);

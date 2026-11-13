// The app's logic that holds no DOM: what a page of the index `byStatus`
// gives a list, and what a person's input writes.

/** A task as a page of the index gives it (RFC 0009, "Get by id"). */
export interface Item {
  readonly title?: string | undefined;
  /** "open", "done", or an option a later model adds, which shows as open. */
  readonly status?: unknown;
  readonly $id: string;
  readonly $space: string;
  readonly $deleted: boolean;
}

/** One line of a list. */
export interface Row {
  readonly id: string;
  readonly space: string;
  readonly title: string;
  readonly done: boolean;
}

/** The lines of a list, in the page's order, without deleted tasks. */
export const rows = (items: readonly Item[]): Row[] =>
  items
    .filter((item) => !item.$deleted)
    .map((item) => ({
      id: item.$id,
      space: item.$space,
      title: item.title || "(untitled)",
      done: item.status === "done",
    }));

/** The status a tick (`true`) or an untick writes. */
export const toggled = (done: boolean): "open" | "done" =>
  done ? "done" : "open";

/** The title an input adds, trimmed; `undefined` for a blank one. */
export const titleOf = (input: string): string | undefined => {
  const title = input.trim();
  return title === "" ? undefined : title;
};

/** A query's page size: 50 by default, at most 1000 (RFC 0009). */
const DEFAULT_PAGE = 50;
const MAX_PAGE = 1000;

/** How many tasks a list shows at first and adds with Show more: `?page=` of the URL. */
export const pageSize = (search: string): number => {
  const asked = Number(new URLSearchParams(search).get("page"));
  if (!Number.isInteger(asked) || asked < 1) return DEFAULT_PAGE;
  return Math.min(asked, MAX_PAGE);
};

/** The next page size of a list after Show more. */
export const more = (limit: number, step: number): number =>
  Math.min(limit + step, MAX_PAGE);

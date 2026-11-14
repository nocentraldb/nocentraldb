// file: db.d.ts
import type { Invitation, Members } from "./members.ts";
import type { Infer, model } from "./nc.ts";
import type { SpaceStatus } from "./status.ts";
import type { SyncStatus } from "./sync.ts";
type AnyCollection = Parameters<typeof model>[0][string];
type Id = string;
interface Elements<E> {
    readonly "nc.elements": E;
}
type Write<T> = [T] extends [readonly (infer E)[] | undefined] ? Elements<E> : Exclude<T, undefined> | null;
type Values<C> = C extends AnyCollection ? {
    [K in keyof Infer<C>]?: Write<Infer<C>[K]> | undefined;
} : never;
type Read<C> = C extends AnyCollection ? Infer<C> & {
    readonly $id: Id;
    readonly $space: Id;
    readonly $deleted: boolean;
} : never;
interface Cursor {
    toString(): string;
}
export interface Query<T, E extends readonly unknown[] = readonly []> {
    equal(...values: E): Query<T>;
    take(n: number): Query<T, E>;
    after(cursor: Cursor): Query<T, E>;
    readonly "nc.query"?: T;
}
export interface Page<T> {
    readonly items: T[];
    readonly cursor: Cursor | undefined;
    readonly hasMore: boolean;
}
type Stop = () => void;
interface Change {
    readonly space: Id;
    readonly collection: string;
    readonly ids: readonly Id[];
    readonly origin: "local" | "remote";
}
interface Written {
    readonly change: string;
    readonly ids: readonly Id[];
}
export interface Tx {
    create<C extends AnyCollection>(collection: C, values: Values<C>): Id;
    update<C extends AnyCollection>(collection: C, id: Id, values: Values<C>): void;
    delete(collection: AnyCollection, id: Id): void;
    restore(collection: AnyCollection, id: Id): void;
}
export interface Space {
    readonly id: Id;
    readonly members: Members;
    readonly erasures: {
        list(): Promise<ErasureEntry[]>;
    };
    readonly status: {
        current(): SpaceStatus;
        subscribe(listener: (status: SpaceStatus) => void): Stop;
    };
    write(record: (tx: Tx) => void): Promise<Written>;
    create<C extends AnyCollection>(collection: C, values: Values<C>): Promise<Written>;
    update<C extends AnyCollection>(collection: C, id: Id, values: Values<C>): Promise<Written>;
    delete(collection: AnyCollection, id: Id): Promise<Written>;
    restore(collection: AnyCollection, id: Id): Promise<Written>;
    get<C extends AnyCollection>(collection: C, id: Id): Promise<Read<C> | undefined>;
    find<T>(query: Query<T, readonly unknown[]>): Promise<Page<T>>;
    subscribe<T>(query: Query<T, readonly unknown[]>, listener: (page: Page<T>) => void): Stop;
    subscribe<C extends AnyCollection>(collection: C, id: Id, listener: (object: Read<C> | undefined) => void): Stop;
}
interface ErasureEntry {
    readonly shred: string;
    readonly state: "final" | "provisional" | "takenBack";
    readonly performer: {
        readonly id: string;
        readonly account: string;
    };
    readonly signOff: string;
    readonly members: readonly {
        readonly id: string;
        readonly account: string;
        readonly current: boolean;
        readonly acknowledged: "deleted" | "seen" | "missing";
    }[];
}
interface SpaceEntry {
    readonly id: Id;
    readonly role: string | null;
}
export interface Db {
    readonly spaces: {
        create(): Promise<Space>;
        get(id: Id): Promise<Space>;
        list(): Promise<SpaceEntry[]>;
    };
    find<T>(query: Query<T, readonly unknown[]>): Promise<Page<T>>;
    subscribe<T>(query: Query<T, readonly unknown[]>, listener: (page: Page<T>) => void): Stop;
    readonly changes: {
        subscribe(listener: (change: Change) => void): Stop;
    };
    invitation(): Promise<Invitation>;
    readonly sync: {
        current(): SyncStatus;
        subscribe(listener: (status: SyncStatus) => void): Stop;
        pause(): Promise<void>;
        resume(): Promise<void>;
        isConfirmed(change: string): Promise<boolean>;
        confirmed(change: string): Promise<void>;
    };
    close(): Promise<void>;
}
interface OpenOptions {
    model: Record<string, any>;
    lock: unknown;
    relay?: string;
    name?: string;
}
export declare const open: (options: OpenOptions) => Promise<Db>;
export declare const add: <E>(...elements: E[]) => Elements<E>;
export declare const remove: <E>(...elements: E[]) => Elements<E>;
export {};
// file: error.d.ts
export declare class NcError extends Error {
    readonly code: string;
    readonly ids: readonly string[];
    readonly fingerprint?: string;
    constructor(code: string, message: string, ids?: readonly string[], fingerprint?: string);
}
// file: index.d.ts
export declare function name(): string;
export type { Db, Page, Query, Space, Tx } from "./db.ts";
export type { Invitation, Notice } from "./members.ts";
export * as nc from "./nc.ts";
export type { SpaceStatus } from "./status.ts";
export type { SyncStatus } from "./sync.ts";
// file: members.d.ts
export interface Invitation {
    readonly fingerprint?: string;
    toString(): string;
}
export interface Notice {
    readonly code: string;
    readonly text: string;
}
type Role = "reader" | "writer" | "admin";
interface Member {
    readonly id: string;
    readonly account: string;
    readonly linkedTo: string | null;
    readonly kind: "device" | "agent" | "subAgent";
    readonly role: Role | null;
    readonly self: boolean;
}
export interface Members {
    add(invitations: Invitation | readonly Invitation[], options?: {
        role?: Role;
        fingerprint?: string;
    }): Promise<{
        readonly notice: Notice;
    }>;
    list(): Promise<Member[]>;
    setRole(member: string, role: Role): Promise<{
        readonly notice: Notice;
    }>;
    remove(member: string): Promise<{
        readonly notice: Notice;
    }>;
}
export declare const invitation: (text: string) => Invitation;
export {};
// file: nc.d.ts
import type { Query } from "./db.ts";
interface Value<T = unknown, D extends boolean = false> {
    readonly "nc.reads": T;
    readonly "nc.deprecated": D;
}
interface Field<T = unknown, D extends boolean = false> extends Value<T, D> {
    personal(options: {
        person: string;
    }): Field<T, D>;
    describe(text: string): Field<T, D>;
    deprecated(reason: string): Field<T, true>;
}
interface Ref<D extends boolean = false> extends Value<Target, D> {
    describe(text: string): Ref<D>;
    deprecated(reason: string): Ref<true>;
}
type Fields = Record<string, Value<unknown, boolean>>;
interface Collection<F extends Fields = Fields, D extends boolean = false, I extends Indexes = Indexes> {
    readonly "nc.collection": F;
    readonly "nc.deprecated": D;
    readonly indexes: {
        readonly [K in keyof I]: Query<Item<F>, EqualOf<F, I[K]>>;
    };
    deprecated(reason: string): Collection<F, true, I>;
}
type Indexes = Record<string, Index>;
type Readable<F extends Fields> = {
    [K in keyof F as F[K] extends Value<unknown, true> ? never : K]?: F[K]["nc.reads"];
};
type Item<F extends Fields> = Readable<F> & {
    readonly $id: ObjectId;
    readonly $space: SpaceId;
    readonly $deleted: boolean;
};
type EqualValues<F extends Fields, K extends readonly string[], N extends number, Taken extends readonly unknown[] = []> = Taken["length"] extends N ? Taken : K extends readonly [
    infer H extends string,
    ...infer R extends readonly string[]
] ? EqualValues<F, R, N, [
    ...Taken,
    Exclude<F[H & keyof F]["nc.reads"], undefined>
]> : Taken;
type EqualOf<F extends Fields, X> = X extends Index<infer K, infer N> ? EqualValues<F, K, N> : readonly [];
type Visible<C> = {
    [K in keyof C as C[K] extends Collection<Fields, true> ? never : K]: C[K];
};
export type Unknown = {
    unknown: string;
};
export type Infer<C extends Collection<Fields, boolean>> = C extends Collection<infer F, false> ? {
    [K in keyof F as F[K] extends Value<unknown, true> ? never : K]?: F[K]["nc.reads"];
} : never;
interface Named {
    readonly "nc.named": true;
}
interface Index<K extends readonly string[] = readonly string[], N extends number = number> {
    readonly "nc.index": true;
    readonly "nc.fields": K;
    readonly "nc.equal": N;
}
type Names<E extends readonly (string | {
    field: string;
})[]> = {
    [P in keyof E]: E[P] extends string ? E[P] : E[P] extends {
        field: infer N extends string;
    } ? N : never;
};
type ObjectId = string;
type SpaceId = string;
type Target = {
    space: SpaceId;
    id: ObjectId;
};
export declare function model<C extends Record<string, Collection<Fields, boolean>>>(collections: C): Visible<C>;
export declare function collection<F extends Fields, const I extends Indexes = Record<never, never>>(fields: F, options?: {
    computed?: Record<string, Named>;
    views?: Record<string, Named>;
    indexes?: I;
}): Collection<F, false, I>;
export declare const string: () => Field<string>;
export declare const int: () => Field<number>;
export declare const float: () => Field<number>;
export declare const boolean: () => Field<boolean>;
export declare const oneOf: <const O extends readonly string[]>(options: O) => Field<O[number] | Unknown>;
export declare const set: <T>(element: Field<T>) => Field<T[]>;
export declare const link: (target: string | readonly string[]) => Field<ObjectId>;
export declare const links: (target: string | readonly string[]) => Field<ObjectId[]>;
export declare const ref: (target: string | readonly string[]) => Ref;
export declare const richText: () => Field<never>;
export declare const stream: () => Field<never>;
export declare const reach: (..._args: unknown[]) => Named;
export declare const view: (..._args: unknown[]) => Named;
export declare const index: <const K extends readonly (string | {
    field: string;
    direction: "asc" | "desc";
})[], const N extends number = 0>(fields: K, options?: {
    equal?: N;
    scope?: "space";
}) => Index<Names<K>, N>;
export { add, open, remove } from "./db.ts";
export { NcError as Error } from "./error.ts";
export { invitation } from "./members.ts";
// file: status.d.ts
export interface SpaceStatus {
    readonly role: "reader" | "writer" | "admin" | "removed";
    readonly initialSyncComplete: boolean;
    readonly behind: number;
    readonly gaps: readonly string[];
    readonly parked: Readonly<Record<string, number>>;
    readonly newestModel?: number;
    readonly rejected: Readonly<Record<string, number>>;
    readonly refused: readonly {
        readonly change: string;
        readonly code: string;
    }[];
    readonly lostGrants: readonly string[];
    readonly equivocations: readonly string[];
    readonly exposed: boolean;
    readonly waitingForKey: boolean;
    readonly badPersonKeyRecords: readonly string[];
    readonly provisionalErasures: readonly ProvisionalErasure[];
}
interface ProvisionalErasure {
    readonly change: string;
    readonly age: number;
    readonly canUndo: boolean;
    readonly performer: string;
    readonly notice: {
        readonly code: string;
        readonly text: string;
    };
}
export {};
// file: sync.d.ts
export interface SyncStatus {
    readonly connection: "connected" | "connecting" | "offline" | "paused" | "none";
    readonly relay?: string;
    readonly unconfirmed: number;
    readonly refused: number;
    readonly epochChanged: boolean;
    readonly error?: string;
}

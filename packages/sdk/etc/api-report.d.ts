// file: cbor.d.ts
export type Cbor = null | boolean | number | string | Uint8Array | Cbor[] | {
    [key: string]: Cbor | undefined;
};
export declare const encode: (value: unknown) => Uint8Array;
export declare const decode: (bytes: Uint8Array) => Cbor;
// file: db.d.ts
import type { Infer, model } from "./nc.ts";
export interface Core {
    call(request: Uint8Array): Promise<Uint8Array>;
    terminate(): void;
}
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
    write(record: (tx: Tx) => void): Promise<Written>;
    create<C extends AnyCollection>(collection: C, values: Values<C>): Promise<Written>;
    update<C extends AnyCollection>(collection: C, id: Id, values: Values<C>): Promise<Written>;
    delete(collection: AnyCollection, id: Id): Promise<Written>;
    restore(collection: AnyCollection, id: Id): Promise<Written>;
    get<C extends AnyCollection>(collection: C, id: Id): Promise<Read<C> | undefined>;
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
    close(): Promise<void>;
}
interface OpenOptions {
    model: Record<string, any>;
    lock: unknown;
    relay?: string;
    name?: string;
}
export declare const open: (options: OpenOptions) => Promise<Db>;
export declare const connect: (core: Core, options: OpenOptions) => Promise<Db>;
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
export type { Db, Space, Tx } from "./db.ts";
export * as nc from "./nc.ts";
// file: nc.d.ts
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
interface Collection<F extends Fields = Fields, D extends boolean = false> {
    readonly "nc.collection": F;
    readonly "nc.deprecated": D;
    deprecated(reason: string): Collection<F, true>;
}
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
interface Index {
    readonly "nc.index": true;
}
type ObjectId = string;
type SpaceId = string;
type Target = {
    space: SpaceId;
    id: ObjectId;
};
export declare function model<C extends Record<string, Collection<Fields, boolean>>>(collections: C): Visible<C>;
export declare function collection<F extends Fields>(fields: F, options?: {
    computed?: Record<string, Named>;
    views?: Record<string, Named>;
    indexes?: Record<string, Index>;
}): Collection<F>;
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
export declare const index: (fields: readonly (string | {
    field: string;
    direction: "asc" | "desc";
})[], options?: {
    equal?: number;
    scope?: "space";
}) => Index;
export { add, open, remove } from "./db.ts";
export { NcError as Error } from "./error.ts";
// file: worker.d.ts
export {};

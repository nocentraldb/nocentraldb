// file: index.d.ts
export declare function name(): string;
export * as nc from "./nc.ts";
// file: nc.d.ts
interface Field<T = unknown, D extends boolean = false> {
    readonly "nc.reads": T;
    readonly "nc.deprecated": D;
    personal(options: {
        person: string;
    }): Field<T, D>;
    describe(text: string): Field<T, D>;
    deprecated(reason: string): Field<T, true>;
}
type Fields = Record<string, Field<unknown, boolean>>;
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
    [K in keyof F as F[K] extends Field<unknown, true> ? never : K]?: F[K]["nc.reads"];
} : never;
interface Named {
    readonly "nc.named": true;
}
interface Index {
    readonly "nc.index": true;
}
type ObjectId = string;
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
export {};

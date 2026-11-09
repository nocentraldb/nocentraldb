// file: index.d.ts
export declare function name(): string;
export * as nc from "./nc.ts";
// file: nc.d.ts
interface Field {
    personal(options: {
        person: string;
    }): Field;
    describe(text: string): Field;
    deprecated(reason: string): Field;
}
interface Collection {
    readonly "nc.collection": true;
}
interface Named {
    readonly "nc.named": true;
}
interface Index {
    readonly "nc.index": true;
}
export declare function model<C extends Record<string, Collection>>(collections: C): C;
export declare function collection(fields: Record<string, Field>, options?: {
    computed?: Record<string, Named>;
    views?: Record<string, Named>;
    indexes?: Record<string, Index>;
}): Collection;
export declare const string: () => Field;
export declare const int: () => Field;
export declare const float: () => Field;
export declare const boolean: () => Field;
export declare const oneOf: (options: readonly string[]) => Field;
export declare const set: (element: Field) => Field;
export declare const link: (target: string | readonly string[]) => Field;
export declare const links: (target: string | readonly string[]) => Field;
export declare const richText: () => Field;
export declare const stream: () => Field;
export declare const reach: (..._args: unknown[]) => Named;
export declare const view: (..._args: unknown[]) => Named;
export declare const index: (fields: readonly (string | {
    field: string;
    direction: "asc" | "desc";
})[], options?: {
    equal?: number;
}) => Index;
export {};

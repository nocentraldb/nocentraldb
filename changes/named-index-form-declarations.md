---
type: breaking
tickets: [NOC-563]
components: [sdk-ts]
---
## Problem
The named index form changes how the TypeScript SDK declares two things. `nc.index` was one arrow function and is now two overloads, one for `nc.index({ where, orderBy })` and one for `nc.index(fields, { equal })`. And `.where(...)` and `.equal(...)` now also take `null` for an unset field. The SDK's API report compares lines, so it lists these declarations as changed.

## Use
Every existing call compiles and behaves as before: `nc.index(fields, { equal })` keeps its exact parameters and result type, and `.equal(...)` takes everything it took before, plus `null`. Code that reads the type of `nc.index` itself, such as `Parameters<typeof nc.index>`, now sees the overload for today's form, which is the last one.

## Migration
None.

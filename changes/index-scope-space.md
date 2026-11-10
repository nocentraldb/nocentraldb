---
type: feature
tickets: [NOC-85]
components: [sdk-ts, cli]
---
## Problem
A schema could not declare an index that runs in one space at a time, so `ncdb model build` never wrote the index key `scope` that the local view reads.

## Use
Give `nc.index` the option `scope: "space"`. `ncdb model build` writes `"scope": "space"` for that index into `model.lock.json`, and leaves the key out for an index that spans every space of the member.

```ts
indexes: {
  byRankInSpace: nc.index(["rank"], { scope: "space" }),
}
```

Adding or removing the option changes the index; the build accepts it as an additive change, gives the index the new model version and lists it as `~ index tasks.byRankInSpace (rank; scope space)`.

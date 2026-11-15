---
type: feature
tickets: [NOC-318]
components: [sdk-rust]
---
## Problem

An agent that hands part of a task to a sub-agent could only add the sub-agent to a space it already had, and so let it decrypt everything in that space, not just what the task needs.

## Use

The agent calls `db.spaces().create_ad_hoc(AdHoc { copy, ..AdHoc::default() })` with the objects the task needs, read from any of its spaces. It gets a new space with a fresh key, holding a copy of each object under a fresh id, and adds the sub-agent to that space with `Members::add`. A link to an object that is copied too points to that object's copy. A link to an object that is not copied is cleared, so no id of the original space reaches the sub-agent. Personal fields and plugin fields are not copied. `AdHocCreated` returns each source with its copy, and `Space::source_of` and `Space::copy_of` find them later, on the agent's own database only, so that it can write the sub-agent's results back. Local ad-hoc spaces and copies of personal fields come later.

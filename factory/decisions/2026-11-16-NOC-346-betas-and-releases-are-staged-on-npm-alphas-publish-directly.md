# 2026-11-16 · NOC-346 · Betas and releases are staged on npm and the owner approves them; alphas keep direct publish

- Decided by: Carsten, on NOC-346 (2026-11-16), option 2 of three, as pm recommended
- Why: a staged version becomes public only when a person approves it with 2FA, on npmjs.com or with `npm stage approve`; no OIDC token can approve. Alphas come after every product-changing merge to `main`: staging them would hold npm's `alpha` back until the owner approves each one and add an owner step per merge. Betas and releases already wait for the owner's tag, so approving them adds no new wait. Staging keeps provenance, and the dist-tag is set when the version is staged. The trusted publisher keeps "Allow npm publish" ticked while `alpha-npm` publishes directly; the staging code comes with the `publish` job's npm step, which does not exist yet.
- Instead of: staging every version, alphas included, so that direct publish could be unticked now (option 1); and staging every version but sending alphas to npm only on a manual dispatch (option 3).
- Topics: release, npm, supply-chain, sdk-ts

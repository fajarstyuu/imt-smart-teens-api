# Final independent whole-change review — global admin and private seeding

## Scope and evidence

This is the single requested independent review, performed inline and read-only. No other reviewer was dispatched. No production command, credential-file read, source/index/HEAD modification, merge, push, service operation, or broad validation suite was performed. This report is the only intentional file write.

Reviewed immutable ranges:

- **B** = `/home/ubuntu/imt-smart-teens/api/.worktrees/global-admin-seeder`, `8e01eda09f04375602109a948f3e17c245810412..2adbb83cd0eee2308af4e94f60de2e29c555e978`.
- **F** = `/home/ubuntu/imt-smart-teens/web/.worktrees/global-admin-seeder`, `e3574bcbd181d1b41d5e5bba9741c8cb8777da4f..a8cbe29d57d7b6f9ce21e3630a5eb007fa7880c7`.
- **R** = `/home/ubuntu/.hermes/cache/scratch/imt-global-production-rollout.py` (standalone operator artifact, not part of either Git range).

Git HEADs matched those supplied and both worktrees were clean at inspection. Read the canonical rules/AGENTS.md, applicable engineering/backend/frontend/delegation/handoff standards, design, plan, both handoffs, changed implementation/tests, and surrounding authentication, report-query, resource lifecycle, catalog and container-entrypoint contracts. The owner-approved single global-admin policy supersedes the former grant model.

Existing execution evidence inspected, **not rerun by this reviewer**:

- `scratch/imt-global-task1-green.log`: programmatically summed **73 passed, 0 failed, 0 ignored** across Rust results; includes the real CLI/global-admin tests. Handoff separately records formatting, Clippy, locked build and infrastructure results.
- `scratch/imt-global-frontend-final.log`: **47 Vitest + 6 legacy tests**, frontend build and actual frontend image build. Two Router module-directive warnings remain.
- `scratch/imt-global-browser-accepted.log`: **9 real Rust/PostgreSQL Chromium cases passed**, including immediate new-school reporting without grants/re-login. The log expressly records the baseline development favicon qualification.
- Actual-container seed/replay/login smoke, backend image build, mobile diagnostics and exact-index readback are reported in the supplied handoffs/context; this review does not relabel those as independently rerun proof.

A proposed in-memory exception-handler fault-injection probe was blocked by the execution-approval boundary and **did not run**. A subsequent pure AST/read-only inspection succeeded: the rollback handler contains subprocess calls but no image/health readback, and neither reviewed release SHA appears as a literal in R. Findings below are source/contract findings, not claims of production failure or successful fault injection. No private credential contents or target identity are reproduced here.

## Strengths

- **Authorization changes are substantive, not cosmetic.** B's session adapter continues checking active accounts and session revocation/expiry on every request (`postgres_identity.rs:135-166`); AdminSession still verifies the admin role and mutation CSRF binding (`extractors.rs:11-51`). Removing boolean gates does not turn student identities into admins.
- **Reporting removes grants at the correct layer.** Student counts/pages/details share an active-admin-guarded source relation (`reporting/infrastructure/students.rs:16-48,69-112`). Answer counts/pages and attempt existence likewise require an active admin (`answers.rs:16-45,143-194`). School/date/type/stage/version filters, stable ordering, repeatable-read transactions and historical snapshots remain intact. There is no enumeration of all school IDs during authentication.
- **Seeder conflicts preserve credentials and identity atomically.** The normalized-email unique-index conflict performs `DO NOTHING`, with distinct Created/ExistingUnchanged outcomes (`postgres_identity.rs:36-52`; `bootstrap/mod.rs:218-231`). Inactive accounts are not reactivated. The unchanged message explicitly says the supplied password was not installed. Real PostgreSQL CLI tests exercise replay, disabled rows, invalid passwords and concurrent normalized duplicates.
- **The frontend is consistent with the clarified role.** Navigation and CMS entry guards no longer consult legacy booleans. The school picker uses bounded 25-school pages, search/status controls, selected-ID resolution, archived names, retry states and stale-response protection; report URL selection remains separate from catalog pagination (`F/src/components/ReportSchoolFilter.tsx:8-31`, `F/src/hooks/useResource.ts:3-6`). The new tests exercise off-page archived selection and preservation of other filters.
- **The operator boundary uses the real existing backend.** `operations.sh:28-35` re-enters `/usr/local/bin/imt-entrypoint` with private stdin and only email/name forwarding; the entrypoint reloads mounted secrets. No competing static-IP one-shot backend is created. Documentation distinguishes seeding from ordinary startup and from credential reset.

## Issues

### Critical — 0

No Critical issue established.

### Important — 2

#### I1. Rollback can fail without establishing or truthfully reporting the resulting production state

- **Location:** R:113-120, especially 115 and 118-120; related image handling at R:45-47,66-69.
- **Evidence:** Both checkout-restoration calls use unchecked `subprocess.run`. The rollback Compose call also uses unchecked `subprocess.run`; its return code is printed but no failure is propagated as a distinct failed-rollback state. There is no readback of checkout SHAs, running old image IDs or service readiness after restoration. The final exception nonetheless says prior settings/images were restored if rollout started. The `images` map contains the new release images, not the exact previous running image IDs, and the helper does not use the previously captured old-image metadata to prove rollback identity.
- **User impact:** A failed deployment followed by a failed checkout or container restoration can leave mixed source/image state or an unavailable service while the operator receives a restoration assertion. Restoring the old settings bytes is not proof the old services are running. This is particularly consequential because rollback is invoked for ordinary public-login/network acceptance failures as well as startup failure.
- **Correction:** Capture/use the exact previous running image IDs and checkout SHAs; attempt each restoration independently, collect failures, and read back the resulting checkout/image/readiness state. Report `rollback verified` only after those checks; otherwise explicitly report `rollback incomplete / operator recovery required` with safe stage-level diagnostics. Do not silently replace the original failure with an unsupported restoration assertion. Add bounded isolated tests for checkout restore failure, Compose restore failure and rollback verification failure; never exercise these against production.

#### I2. The rollout helper does not bind its deployment to the reviewed source/image release

- **Location:** R:45-47,56-63,66-69,73.
- **Evidence:** `release_heads` is populated from whatever `git rev-parse HEAD` returns in the feature checkouts at execution time; cleanliness is checked, but equality to the reviewed B/F SHAs is not. New image IDs are likewise accepted from the mutable release tag at execution time. The later running-image assertion only compares the container with that just-observed tag value, not the independently built/reviewed release identity. The seed wrapper is then loaded from the live feature path. Pure AST inspection confirmed neither reviewed SHA is pinned in the artifact.
- **User impact:** A clean commit change or tag replacement between review/build and operator execution can silently deploy a different release, or pin source checkouts to code inconsistent with the installed images. The helper would still pass its existing image assertions. No actual drift was observed during this review; the defect is that the promised reviewed-release restriction is not enforced at the production mutation boundary.
- **Correction:** Supply an explicit approved release manifest (both exact source SHAs and both expected built image IDs/digests), reject any mismatch before the first checkout/settings/service write, and deploy those immutable image identities. Execute the matching pinned wrapper or verify its revision. Update that manifest deliberately if the parent's authorized material-fix pass changes the accepted heads/images. Add a small isolated preflight regression proving that clean-but-unreviewed HEADs and moved image tags stop before mutation.

### Minor — 1

#### M1. Source-level authorization contracts still describe the removed school-grant policy

- **Location:** B/src/modules/reporting/application.rs:195-196; B/src/modules/identity/application/schools.rs:1.
- **Evidence:** ReportReader's public documentation says every root query joins live grants; the school-use-case module says permissions are enforced separately from reporting grants. The current implementations intentionally do neither.
- **User impact:** Maintainers reading the owning interfaces can reconstruct the superseded authorization model or mistakenly reintroduce grant filtering. Runtime behavior is correct; this is documentation drift, not an access leak.
- **Correction:** Describe active global-admin identity checks and distinguish school data filters from authorization. Retaining legacy storage does not require documenting it as effective authorization.

## Explicit acceptance review

| Required focus | Assessment |
| --- | --- |
| 1. Legacy false-booleans/no-grants admin is global; student/anonymous/inactive sessions stay denied | **Accepted at implementation-review level.** Shared CMS/domain authorization and SQL actor checks match the policy; global_admin.rs and retained session/HTTP tests cover positive and denial paths. |
| 2. New/archived schools, totals/offsets, historical answers and filters | **Accepted at implementation-review level.** Global source relations do not depend on grant rows or active-school catalog status; new/archived, count/offset, historical-version and date/stage tests remain. |
| 3. Private idempotent/concurrent seeding preserves old identity/hash/name/active state | **Accepted at implementation-review level.** Atomic unique-index insertion, explicit unchanged semantics, private argument discipline, Argon2id and real CLI tests support the contract. Production creation/login remains unperformed. |
| 4. More than 25 schools, archived/off-page selected URL IDs, bounded reads/filter preservation | **Accepted at implementation-review level.** Picker/service/resource code and new component tests cover the stated behavior; real browser evidence covers new-school access and mobile overflow correction. |
| 5. Existing-container entrypoint with stdin, not a conflicting one-shot container | **Accepted at implementation-review level.** Exact wrapper invocation and entrypoint loader inspected; stricter sudo-reset contract test and actual smoke evidence are recorded. |
| Planned production rollout helper | **Not accepted unchanged.** Resolve I1/I2 before executing production mutation. Production HTTPS acceptance, data/count readback and private credential delivery are still pending operations, not completed implementation evidence. |

No schema migration/drop, grant backfill, account-management UI or authorization-tier expansion was introduced. The native-implementation/routing limitation is disclosed rather than misrepresented as a successful Luna delegation.

## Recommendations

1. Keep the material correction pass scoped to the two rollout-helper findings; cover the fail-closed manifest and rollback outcomes with isolated tests. M1 is a small owning-comment correction, not grounds for unrelated cleanup.
2. Parent should adjudicate these findings and update exact accepted SHAs/image identities if any authorized fix changes the artifacts. Do not perform another broad review or infer merge/push authorization.
3. Before the separately authorized rollout, verify the private backup/catalog and immutable release manifest. After mutation, read back exact service/image/account targets and execute the public HTTPS login/global routes/logout and unchanged teaching/schema counts. Only then publish the private delivery artifact. Do not call smoke evidence production proof.

## Declined to judge

Every considered behavior set aside is listed here, before the verdict:

- **Per-school administrators, superadmin tiers or selective-admin grants:** deliberately rejected by the owner; evaluating their absence as a defect would contradict the accepted role model.
- **Dropping legacy columns/grant tables or rewriting historical migrations:** explicitly deferred compatibility cleanup; retaining inert data is the approved non-destructive strategy.
- **Admin account-management UI, password-change/reset UI, account renaming, MFA and public registration:** no such new workflow was requested; private create-only/idempotent provisioning must not silently become account recovery or registration.
- **Automatic retry after a created account survives a failed rollout:** the helper intentionally refuses an existing target and preserves it rather than overwriting. Manual operator reconciliation is required; automatic password rotation/deletion would exceed authorization. This does not excuse I1's unverified rollback assertions.
- **Specific target identity, actual password, prepared word-list contents/entropy and private credential-file permissions:** operator-only artifacts were outside the allowed read scope; this review does not certify their values or generation. The design's generation and final private-delivery requirements still belong to the operator's acceptance checklist.
- **Actual production rollout success, public HTTPS credential authentication and delivery:** intentionally not executed in this read-only review. They remain required pending acceptance work, not waived requirements or claimed successes.
- **Independent off-host/external-network reachability, disaster recovery, governance/retention/compliance approval and capacity/load guarantees:** broader pre-existing operational qualifications, not established by the scoped synthetic tests or local review. No new certification is implied.
- **A new native 200% zoom/full assistive-technology/browser-matrix audit:** not performed here or claimed by the handoff; prior evidence is not relabeled as this change's proof. Existing accessibility obligations remain.
- **Baseline development favicon installation, two existing Router build warnings and unrelated Minor backlog:** disclosed pre-existing qualifications, not new global-admin regressions. The exact favicon-only exception was inspected; it is not permission to waive API/page failures generally.
- **General editor/history-navigation redesign, source formatting overhaul and additional catalog UX optimization such as debouncing:** no material new defect established in those unchanged/working behaviors; the new picker is bounded and discards stale responses.
- **Merging/pushing main, deleting stable runtime worktrees, rotating secrets, modifying production data or adding teaching fixtures:** outside explicit authorization and unnecessary to this review.

## Verdict

- **Counts:** 0 Critical, 2 Important, 1 Minor.
- **Spec acceptance:** backend global-admin semantics, global-report frontend and private idempotent seeder accepted at code-review level; production rollout helper requires the two material corrections above. Production acceptance is pending, not passed.
- **Quality:** focused implementation with real PostgreSQL/CLI/browser regression evidence, preserved privacy/session boundaries and appropriately bounded catalog reads. Operational failure handling and exact release provenance need strengthening before the irreversible account-creation/deployment workflow is run.
- **Ready: With fixes.** Do not execute the current production rollout helper unchanged. Parent adjudication plus the single bounded material-fix pass is the next action; this review itself changes no implementation or production state.

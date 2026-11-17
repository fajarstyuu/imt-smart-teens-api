# Global admin and private seeder — review handoff

- Date: 2026-11-17, runtime UTC+08:00.
- Status: parent material correction verified after the single independent review; production rollout/provisioning pending.
- Owner approved one global admin role (no per-school admin/superadmin tier), idempotent private seeding and production rollout. Native implementation with one final independent whole-change review; no new source merge/push authorized.
- Bases: API8e01eda09f04375602109a948f3e17c245810412; FEe3574bcbd181d1b41d5e5bba9741c8cb8777da4f. Current branches feat/global-admin-seeder under each .worktrees/global-admin-seeder. Backend implementation9aa1ccb/operatorceeb91c; final review heads recorded by Git.
- Specs read: rules/AGENTS.md, docs/{rules,backend,frontend,delegation,handoff}.md and this feature design/plan. The owner's clarified global-role contract supersedes the old reporting grant policy, not student privacy/session protections. Implementer Mochi/session model; no per-task delegates or invented model/effort routing.

## Scope and contracts

All authenticated active admins have global CMS/report access across new/existing/archived schools. Legacy flags/grant rows remain untouched compatibility storage, not authorization gates. Wire capabilities are true and school_ids deprecated/empty; frontend uses bounded searchable catalog pages plus selected-ID detail reads, not session school enumeration. Report SQL includes an active-admin actor check before counts/pages/details and retains filters, stable ordering, immutable snapshots, submission/answer semantics and explicit enrollment selection. Student, anonymous, inactive, revoked and expired sessions, CSRF and audit enforcement remain.

Private seed-admin command shares normalized email/name validation and Argon2id hashing, atomically INSERT ON CONFLICT(lower(email)) DO NOTHING; Created versus ExistingUnchanged is explicit. Existing identity/hash/name/active state are preserved, including inactive rows and concurrent seeds. Existing provision-admin remains create-only. No schema migration, automatic grants, default account, public admin registration or serve-time seeding.

Operator action uses existing backend /usr/local/bin/imt-entrypoint with stdin; no competing static-IP container. Only nonsecret ADMIN_EMAIL/ADMIN_NAME are preserved through sudo; password never enters Docker arguments/config/logs. New real smoke-only integration test executes this path and validates creation/replay/global access/logout with stored legacy flags=false.

## Actual evidence

- Baseline69Rust/44Vitest+6legacy passed. Task1 meaningful RED: catalog403 for legacy-false admin; seed-admin unknown command failed2controls. GREEN:73Rust/0failed/0ignored, fmt/warnings-deniedClippy/lockedbuild;19infra after operator additions (initial16).
- Frontend3new behavior tests RED for hidden menus/missing catalog selector/error recovery; GREEN47Vitest+6legacy/build. Two existing React Router bundling warnings remain disclosed.
- Operator script RED missing command; additional sudo-env-reset control reproduced missing email, then GREEN3/full19infra; sh syntax check passed. Actual independently named smoke stack seeded one synthetic admin, replay preserved ID/hash, HTTPS-origin/private socket login201 and global legacy-false account reads200, logout204/then401. This is not production proof.
- Full real Rust/PG Chromium suite first exposed new picker intrinsic-width overflow (360px viewport/document391px). Controlled browser diagnostics proved the cause, then scoped grid/min-width CSS produced360/360 with no external overflow. A Playwright exact-label ambiguity was corrected in test code, not suppressed as product success.
- The next run exposed a baseline full-Chromium /favicon.ico404, traced to that exact browser-owned URL (not an API404); baseline FE Git tree has no favicon. The harness logs/excludes only that exact same-origin development asset, keeping API/page errors failing. It is a disclosed qualification, not a fixed icon.
- Final complete9real Rust browser tests passed (scratch/imt-global-browser-accepted.log), including complete student journey, all prior draft/unknown-ack safeguards, immediate global new-school reads without grants/re-login and desktop/mobile screenshots. New mobile report390px screenshot visually inspected; no horizontal document overflow, internal table horizontal scrolling remains intentional. Evidence under frontend test-results/evidence/global-admin-new-school-{desktop,mobile}.png. Native200% zoom was not re-run for this change; previous milestone evidence is not relabeled.
- Both real digest-pinned Docker images built serially under global-admin-20261117. Smoke frontend readback equals exact latest built index. No production containers restarted yet. Production read-only preflight:0admins/schools/students/lessons,5migrations and requested account absent. Private passphrase prepared but never printed/seeded/committed. Private production dump and prior settings/image metadata captured for authorized rollout.

## Rulings and costs

1. Retain legacy flags/grant rows inert rather than destructive schema cleanup — user corrected role semantics, not stored history — cost: later explicit schema cleanup remains.
2. Default display name Admin and keep target identity/password in operator-only inputs — owner supplied no personal display name — cost: a separate display-name edit may be requested.
3. Extend Task2 ownership to scoped teacher.css grid rules — actual new picker caused mobile overflow — cost: report toolbar layout changes; real360/390px and full-suite evidence cover the change.
4. Log/exclude only exact same-origin development favicon404 from the API/page-error assertion — browser-owned asset is absent in baseline; no API errors waived — cost: tab-icon behavior remains untested/unfixed, explicitly not production functional proof.
5. Preserve nonsecret seed email/name through sudo's environment reset — fake echo-sudo was insufficient and the stricter control observed failure — cost: operator sudo policy must permit those two environment variables; never preserve passwords/database URLs through this route.
6. Pin existing stable deployment checkouts to reviewed release SHAs in detached mode at rollout, retaining paths/ignored state — avoids broken bind mounts or build contexts pointing at stale code — cost: do not delete these runtime checkouts; main and feature branches are not merged/pushed by this operation. Rollback restores old checkout SHAs/settings/images.

## Final review disposition and verified correction

ONE independent review sa-0-754a61ef/deleg_b2394bce returned With fixes:0Critical/2Important/1Minor. Backend/global frontend/seeder accepted at implementation-review level; original rollout helper rejected unchanged. Report preserved under docs/reviews/2026-11-17-global-admin-review.md. Optional reviewer fault probe was approval-blocked and did not run; findings came from source/evidence, not alleged production faults.

Parent accepted I1/I2 and performed one bounded correction. deploy/release_guard.py plus five isolated controls define fail-closed approved source/tag identity, independent checkout/settings/image restoration and exact old SHA/image/readiness/settings readback. A failed action or verification yields explicit incomplete/operator-recovery status, never an unsupported restored assertion. Tests first failed for missing release boundary, then all5passed/full24infra; no production fault injection. Private operator helper now consumes an explicit manifest of exact source SHAs, built image IDs, previous SHA/image targets and its own file digest, validates outside the rollback/mutation path, deploys immutable image IDs via protected Compose override, and re-enters the pinned stable seeder wrapper. Rollback uses immutable old IDs and independently checked restoration results with safe phase diagnostics. Final73Rust/0failed/0ignored/fmt/Clippy/lockedbuild and47Vitest+6legacy/build rerun passed; no Rust/runtime frontend source changed in this helper-only correction, so accepted image artifacts remain applicable. Actual guarded preflight/rejection checks and production operations remain the next steps, not claimed here. No second reviewer approval is invented.

Final: minor (deferred) M1: two owning source comments still describe the superseded school-grant model (reporting/application.rs interface comment and identity/application/schools.rs module comment). Runtime global behavior is correct; cost is maintenance confusion until a separately scoped documentation correction. Existing baseline favicon/Router and broader old backlog qualifications remain disclosed.

## Declined-to-judge rulings and costs

7. No per-school/superadmin/selective-admin tier — explicitly rejected by owner — cost if needs change: a new role contract/migration, not hidden grants.
8. No dropping legacy fields/tables or rewriting migrations — preserve data and compatibility — cost: explicit cleanup later.
9. No account-management/reset/rename/MFA/public registration UI — not requested and would broaden scope — cost: separate authenticated workflows are still needed for recovery/security enhancements.
10. Preserve an account surviving failed rollout; stop for reconciliation rather than rotate/delete/retry automatically — prevents credential loss — cost: manual recovery before another attempt.
11. Private identity/passphrase/permissions are operator acceptance, not reviewer-certified — reviewer could not inspect them; parent used OS CSPRNG263unique familiar words/sixchoices/numeric suffix and0600 files — cost: final real login and secure delivery must succeed before any credential claim.
12. Public production login/delivery remain operations, not inferred from smoke — cost: keep deployment incomplete until actual acceptance.
13. Off-host availability, full disaster recovery, governance/retention/compliance and capacity certification remain outside scope — cost: separate audits/tests before those claims.
14. No new native200% zoom/assistive technology/browser-matrix certification — old evidence is not relabeled — cost: specialized audit still required.
15. Baseline dev favicon404, two Router warnings and unrelated Minor backlog remain explicit — no new material regression established — cost: separate polish/maintenance; API/page failures never waived.
16. No editor/history redesign, format overhaul or picker debouncing — existing working flows remain tested and reads bounded/stale-safe — cost: future UX optimization may be warranted.
17. No main merge/push, stable-worktree deletion, secret rotation or teaching fixtures — outside this feature authorization — cost: owner integration/provisioning choices remain.

## Pending acceptance

After the approved manifest passes actual read-only preflight and rejects deliberately mismatched manifest copies without changing live state: pin stable paths, select immutable reviewed images, recreate only backend/frontend, seed through the real operator wrapper, read back one unchanged-on-replay account, verify actual public HTTPS login/admin routes/logout and unchanged teaching/schema counts. Only then create/deliver private admin-access.txt. Account identity/password remain out of this document and Git. Existing broader governance/accessibility/capacity/external-network limitations and prior unrelated Minor backlog remain; report-only UUID fallback necessarily disappears because all admins can use the version picker.

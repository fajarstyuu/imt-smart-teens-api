# Global Admin and Private Seeder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (recommended, preserving the previously selected native method) or superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Make all active admins global and safely provision the owner's requested account with a generated, verified password.

**Architecture:** Keep the Rust modular monolith and server-side cookie/session authorization. Replace grant/boolean authorization with active-admin authorization, retain legacy storage as deprecated compatibility data, and expose explicit idempotent operator-only seeding. Frontend consumes global capabilities and bounded school catalog pages.

**Tech Stack:** Rust1.100.0/Axum/SQLx/PostgreSQL17, React/TypeScript/Vite, existing Docker Compose and host Nginx; no new production services.

**Spec:** docs/superpowers/specs/2026-11-17-global-admin-seeder-design.md

## Global Constraints

- English engineering artifacts, Indonesian UI; role name admin, not guru or superadmin.
- Preserve active-session checks, CSRF, rate limits, audits, typed boundaries, immutable snapshots and student ownership.
- Preserve legacy schema/data; no grants backfill, destructive migration or secret/password rotation.
- Production secrets stay private; no plaintext/email default baked into code or artifacts. Production provisioning requires owner confirmation of this plan.
- Prepare feat/global-admin-seeder in new isolated API/frontend worktrees; bases8e01eda09f04375602109a948f3e17c245810412 and e3574bcbd181d1b41d5e5bba9741c8cb8777da4f. Move these two planning artifacts into the backend worktree before baseline tests, restoring clean root main; preserve old runtime worktrees. No automatic merge/push.

## Review Focus

1. Legacy admin with false permission booleans/no grants gains all intended CMS/report access; student/anonymous/inactive sessions do not. Task1 tests pin both sides.
2. School created after login, archived school, count/offset and historical answer routes remain globally accessible and filtered correctly. Task1/2 tests pin these cases.
3. Seeder rerun/concurrent normalized duplicate does not overwrite hash, identity or active state, and does not claim a new password works for an old account. Task1 CLI tests pin these cases.
4. New school picker handles more than25 schools, archived entries and selected URL IDs outside the current page without dropping filters or unbounded fetching. Task2 tests pin these cases.
5. Docker exec does not inherit entrypoint-generated secret env; seed must call /usr/local/bin/imt-entrypoint inside the existing backend with private stdin, not start a conflicting static-IP Compose backend. Task3 smoke pins actual invocation and redacted outcomes.

## Task1 — Global backend authorization and idempotent private seeding

**Files:** Modify src/modules/identity/{domain.rs,application/content.rs,infrastructure/postgres_identity.rs,presentation/dto.rs}, src/modules/reporting/infrastructure/{students.rs,answers.rs}, src/bootstrap/mod.rs; create src/bootstrap/admin_seed.rs if needed to keep bootstrap focused. Update tests/{reporting.rs,http.rs,sessions.rs,admin_content.rs,cli.rs}, add tests/global_admin.rs, and update docs/runtime-contract.md/current deployment guidance.

**Interfaces:** Every active AdminPrincipal represents a global admin. Existing session authentication remains the source of that principal. AdminDto retains legacy booleans=true and deprecated school_ids (not a scope); no unbounded all-school list. Existing provision_admin remains create-only. Add a typed SeedAdminOutcome::{Created,ExistingUnchanged} and an atomic repository insert-on-conflict operation returning that outcome. CLI seed-admin accepts no extra arguments, requires private credentials and reports outcomes without sensitive fields.

- [x] Write real PostgreSQL/HTTP regression test active_admin_is_global_without_legacy_grants: initialize two schools/cohorts, log in an admin with both stored booleans=false/no grants, assert CMS access and report totals/details/attempt and historical-question answers across both; create a third school after login and assert immediate access; archive one school and retain its historical results. Assert school/date/stage/version filters and bounded offsets still work. Anonymous/student/inactive/revoked sessions must remain denied. Run cargo test --test global_admin --locked -- --test-threads=1; expect old gate/scope assertions to fail, not infrastructure errors.
- [x] Write real CLI tests seed_admin_is_idempotent_and_private and seed_admin_concurrent_normalized_email_preserves_identity: production settings with private stdin, valid/invalid passwords, no password CLI arguments, inactive/conflicting existing rows unchanged, repeated/concurrent requests leave exactly one ID/hash, stdout/stderr never contain submitted passwords. Run focused tests RED; new command currently fails.
- [x] Implement active-admin-only authorization and replace grant joins in every reporting relation/existence check; retain an active-admin actor guard in SQL where the admin UUID is the repository input. Do not merely make /me flags true or grant current schools. Stop loading grant lists as session scope. Remove obsolete authorization branches and update legacy tests to the newly approved contract while retaining non-admin/CSRF/audit/rollback assertions.
- [x] Implement private seed-admin using existing ArgonPasswords/email/name validation. Insert atomically; distinguish Created from ExistingUnchanged, never update an existing account or print secrets. Retain strict command parsing and existing create-only provision-admin semantics.
- [x] Run focused GREEN, then cargo fmt --all --check, cargo clippy --all-targets --all-features --locked -- -D warnings, cargo test --all-targets --locked -- --test-threads=1, cargo build --locked, and complete infrastructure unittests with the existing private owned test environment. Expect0failures/ignored; record actual totals and commit exact scoped files.

## Task2 — Global frontend and bounded school report filtering

**Files:** Modify src/pages/{TeacherResultsPage.tsx,AdminSchoolsPage.tsx,TeacherContentPages.tsx}, src/components/TeacherShell.tsx, src/services/{dto.ts,admin.ts} as needed; create src/components/ReportSchoolFilter.tsx and tests/global-admin.test.tsx; update tests/{admin-components.test.tsx,rust.spec.ts} and README.

**Interfaces:** Consume active authenticated admin from Task1, not legacy flags/school_ids for gating. ReportSchoolFilter consumes selected:string and onChange:(id:string)=>void; fetch pages through adminApi.schools(q,status,offset), page size25, use adminApi.school(id) for an absent selected entry. Include active/archived filters, names not UUID-only display, and keep empty value as all schools.

- [x] Write global_admin_navigation_and_reports Vitest regression with legacy false booleans/empty school_ids, all CMS tabs, copy stating all schools, paginated26+school fixtures, archived entries, preserved off-page selected URL IDs, loading/error/retry and stable report filters. Run npm test -- --help only if needed to discover scripts; run npx vitest run tests/global-admin.test.tsx for meaningful RED.
- [x] Remove obsolete frontend permission gates and report-only UUID fallback branch; replace scoped copy and picker with the bounded component. Preserve router/draft guards, Indonesian copy, visual tokens and existing report pagination/version selection. Do not add account-management UI.
- [x] Convert the old real-browser test 'catalog/content administration does not grant reporting scope for a new school' into positive global behavior. Remove E2E_GRANT_HELPER dependence/grant mutation from the runtime browser suite; retain unauthorized student assertions. Validate new-school read immediately after creation with the same admin session.
- [x] Run focused GREEN, complete npm test/npm run build and the real Rust-browser suite against a separately owned synthetic backend using Task1 source, honoring existing test server ownership/private environments. Capture desktop/mobile admin results/schools proof; no production fixtures. Commit exact scoped changes and actual evidence.

## Task3 — Private operator workflow, review and authorized rollout

**Files:** Modify deploy/operations.sh and docs/deployment.md, add tests/infra/test_admin_seed_operation.py; add dated handoffs in both repositories. Keep credential generation/execution in private operator state outside Git.

**Interfaces:** operations.sh seed-admin requires nonsecret ADMIN_EMAIL/ADMIN_NAME, explicitly runs compose exec -T -e ADMIN_EMAIL -e ADMIN_NAME backend /usr/local/bin/imt-entrypoint seed-admin; stdin carries password. It never accepts a password argument or default production account. Existing backend is reused, avoiding a static-IP collision. Ordinary up/serve/migrate do not seed.

- [x] Write mocked shell contract regression for the exact stdin-preserving exec invocation, missing email/extra arguments, no implicit seeding or Docker plaintext-password env/config. Observe RED, implement and run full infra GREEN.
- [x] Generate the passphrase only after the account preflight shows absent, using OS CSPRNG and a validated unique >=128-word familiar ASCII Indonesian list, >=6 independent word selections plus a random numeric suffix. Keep value in memory/private0600 storage only. Existing account is never overwritten; if present, report unchanged and ask about a separate authorized reset instead of publishing a non-working password.
- [x] Obtain ONE independent whole-change review of both ranges and all five focus classes. Parent adjudicates, fixes only material findings in one RED/GREEN pass and reruns whole gates; record remaining limitations. Do not dispatch repeated broad reviews or fix unrelated backlog.
- [x] After explicit production approval, privately back up production; capture old image IDs and safe nonsecret mount/context metadata. Build both reviewed images serially using explicit feature build contexts and a new release tag. Preserve the original production settings and stable checkout bind mounts: do not silently migrate live paths to disposable worktrees. Validate Compose secrets/networks/ports, then recreate only required frontend/backend services with the new images, retaining PG volume/secrets/domain policy. Restore prior images/settings if readiness/public checks fail.
- [x] Run the private seeder in the real production backend, read back exactly one matching active global-admin account and unchanged teaching/student data. Re-run seeder and prove same identity/hash; never print hashes/passwords. Authenticate over public HTTPS through FE proxy in an in-memory HTTP client (credentials never browser DOM or screenshots), verify /admin/me, schools/content/report routes and logout. Only after successful login deliver the actual generated password to the owner as requested, never to committed handoffs. Do not invent a password-change UI.
- [x] Verify public TLS/FE/API403/portfolio and healthy3containers. Finalize handoffs with exact branches/SHAs/image IDs and real gates, no new merge/push without instruction; preserve required runtime worktrees.

## Plan self-review

The design maps to Task1 backend/seeder, Task2 frontend, Task3 operator/privacy/review/rollout. All five failure classes have named test steps. Existing provision-admin is already callable under production settings; do not claim or bypass a nonexistent environment guard. Only init-content-config has the development-only guard, and it is unchanged. No migration or synthetic production content is required. Report query actor checks and selected archived-school pagination are explicit, not inferred from /me booleans. Password delivery remains blocked on actual created-account login verification.

# IMT Rust Backend and Frontend Integration Implementation Plan

> For agentic workers: use superpowers:subagent-driven-development. The owner explicitly approved Sol plus available-model implementation subagents for this new milestone. Per-child Luna/high routing is unavailable; record actual metadata rather than claiming it. Execute sequentially across dependent contracts, with independent read-only task reviews.

**Goal:** Implement the persisted Rust API described in the approved inventory/database design, connect the existing React frontend, and verify the complete student/admin workflow against real PostgreSQL.

**Completion (2026-11-16):** Tasks1–5 locally accepted after one final consolidated correction and scoped re-review. Seven explicit Minor items and separate production qualifications remain. See `docs/handoffs/2026-11-16-full-stack-final.md` for evidence, all rulings/costs and owner integration decision. No feature merge/push/public rollout.

**Architecture:** One Axum/Tokio modular monolith and SQLx/PostgreSQL, with inward domain/application dependencies and explicit infrastructure adapters. Preserve the existing React/Editorial Campus application, replace its Node-development API integration with cookie/CSRF-based v1 calls, and add the admin school interface. This is a new milestone after the PostgreSQL foundation; its previous no-business-migrations limitation does not restrict this newly authorized implementation.

**Spec:** `docs/api-inventory.md`, `docs/database-design.md`; canonical rules `/home/ubuntu/imt-smart-teens/rules/AGENTS.md`, `docs/rules.md`, `docs/backend.md`, `docs/frontend.md`, `docs/delegation.md`, `docs/handoff.md`, relevant handoffs. User requires admin (not teacher), school add/edit/delete, shared paired pre/post bank, separate quiz bank, sequential lessons, BMI and monitoring.

## Global constraints

- No push, public deployment, nginx change, data destruction, or merge of the new feature branches without owner authorization. The owner authorized the already-completed foundation merge into local main only.
- Main backend now d3327ae49ed7dd153662ec29d432fb78470c9f43; backend feature worktree `/home/ubuntu/imt-smart-teens/api/.worktrees/full-backend`, branch `feat/full-backend`.
- Frontend source base 6344f0b7304d9cfcd5b642034ccf33b2c3393585; future frontend worktree `/home/ubuntu/imt-smart-teens/web/.worktrees/rust-api-integration`, branch `feat/rust-api-integration`.
- Rust 1.100.0 verified from official stable manifest and installed under `/home/ubuntu/.cargo/bin`; use this exact toolchain, lock dependencies. Set PATH explicitly in commands.
- Host RAM about 2 GiB. Use `CARGO_BUILD_JOBS=1`, dev/test debug info 0, limited DB pool, and serial integration tests. Do not launch competing Rust builds or browsers.
- Existing PostgreSQL service remains in foundation worktree/project `postgres-foundation`, port 127.0.0.1:55432. Do not recreate it, delete its volume, or change its Compose project name during feature work.
- Parent provisions isolated application/migrator/test roles and private ignored `.env` / `.env.test`. Never print credential files, database URLs, tokens or passwords. No application superuser connection.
- API listener loopback 127.0.0.1:4180; browser dev server loopback 127.0.0.1:5173. No public exposure in this task.
- All source/comments/docs English; UI and user-facing API messages Indonesian. Preserve current visual design and router behavior. UI labels/roles say admin.
- No mocks as evidence of real persistence. Use real PostgreSQL tests. Never skip required DB tests silently.
- Student login requires an active school ID; name/school/date do not recover an existing identity. Archived schools cannot enroll new students, but preserve history and existing sessions.
- Pre/post share one immutable snapshot per enrollment; quiz separate. No answer-key leaks or locked lesson bodies. Server enforces ownership, school scopes and prerequisites for reads and writes.
- Sessions are opaque cookie-based with expiration/revocation, production Secure; CSRF token plus allowed Origin on mutations including login. CORS explicit and credentialed, never wildcard.
- Serialize autosave/submission through attempt row locking and answer revisions. Score/submission/progress and idempotency effects commit together. No business logic or raw SQL in HTTP handlers; domain/application types do not import SQLx/Axum/env.
- School deletion is archive; no destructive hard-delete route. Versioned content publications affect future enrollments only.
- No public production seed credentials. Tests use clearly synthetic data. A development-only bootstrap can provision example content/admin through an explicit CLI; it must refuse production mode and never print/store plaintext passwords in committed files.

## Review focus

1. Invalid/expired cookie, missing/bad CSRF and unauthorized school/resource access must fail without leaking private records.
2. Admin content edits and school rename/archive must not rewrite existing attempt snapshots or enrollment history.
3. Concurrent autosave/submission/idempotent retry must preserve exactly one result and committed progress.
4. Frontend error/loading/retry must preserve entered answers/forms; 204 must not be JSON-parsed; answer revision is not question version.
5. Browser reload must restore actual persisted state; test against Rust, not the existing Node fixture API.

## Task 1 — Backend foundation, real migrations, identity and school CMS

**Ownership:** Backend worktree `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml` if required, `src/`, `migrations/`, `tests/`, `.env.example`, `.gitignore`, backend documentation needed for these features. Do not edit parent-owned plan, orchestration ledger or `docs/handoffs/2026-11-15-full-stack.md`. Do not edit frontend/rules/foundation worktree or host configuration.

**Interfaces produced:** Thin `src/main.rs` -> bootstrap, testable library router, validated typed settings, PgPool/adapters, shared API error/envelope/pagination, owned principal/session extractors. Identity domain/application ports and PostgreSQL implementations; publish exact constructors/types and DTO examples in task report so Task 2 reuses them instead of duplicating.

- [x] Read canonical specs and existing inventory/database design before editing. Write a short implementation contract report naming modules and reusable interfaces.
- [x] Write failing focused tests for domain normalization and positive-value/date rules used here; failing HTTP tests for liveness/readiness, cookie/CSRF/roles, and school create/edit/archive/revision/active-list behavior. Run and record meaningful RED failures, not merely missing tooling.
- [x] Implement one Cargo package `imt-smart-teens-backend`, pinned toolchain, small modules and meaningful comments; proper bootstrap lifecycle/config/telemetry/router/shutdown. Do not put all code in one giant file.
- [x] Create append-only SQLx migrations implementing the full 28-table relational design with explicit referenced composite UNIQUE constraints and FK/index ordering. Use parameterized runtime SQLx queries initially to avoid needing generated offline metadata. Test empty migration and ownership/uniqueness/history RESTRICT constraints using the isolated test DB. No JSONB replacement for core relational models.
- [x] Implement `/health/live`, `/health/ready`, `GET /api/v1/config`, CSRF context endpoint, admin sessions/current/me and public/admin school endpoints exactly as inventory. Protect mutations with CSRF+Origin, role and explicit permission; normalize errors safely, no secret logging. Use Argon2id and hashed session tokens, expirations, revocation, safe cookies and consistent no-store responses.
- [x] Implement explicit private CLI commands for migrations and admin provisioning; migrations use MIGRATION_DATABASE_URL, server uses DATABASE_URL. Admin provisioning reads a password from environment/stdin securely, never CLI plaintext arguments or public defaults. No student-derived admin accounts. A school-management permission does not grant access to all results.
- [x] Tests pin: name blank/overlong/Unicode-equivalent duplicate; active vs archived picker; stale revision 409; hard history remains after archive; missing cookie 401; wrong role/scope/CSRF 403; unknown/not-owned 404; invalid identity inputs 422; readiness when DB unavailable 503; JSON rejection/404/405/error response shape; correct and incorrect admin login, logout revocation.
- [x] Run `cargo fmt --all --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo test --all-targets --locked -- --test-threads=1`, `cargo build --locked`. Set environment from ignored `.env.test` privately and ensure PostgreSQL tests actually run. Do not apply migrations to application DB yet (parent integration later). Keep test cleanup confined to owned test databases.
- [x] Commit scoped files with conventional commits, self-review, write task report with exact test commands/results, public interfaces and any incomplete inventory endpoints. Return ready_for_review, not whole-app complete.

## Task 2 — Persisted student workflow

**Ownership:** Backend feature worktree only, primarily `src/modules/learning/`, `assessment/`, `anthropometry/`, identity registration, shared application transaction ports, focused persistence adapters and `tests/`. Reuse Task 1 types. Update contracts documentation if a concrete field detail needs clarification, and tell parent.

- [x] Read Task 1 report/source/interfaces. Specify exact response DTOs for frontend integration in `docs/runtime-contract.md`; use inventory names, stable UUIDs and nullable scores, not the old fixture payload.
- [x] RED tests: student registration with active school, unavailable content fails atomically, pre-test gating, sequential lessons/required section visits, quiz/calculator/post/summary sequence; another student's attempt is inaccessible.
- [x] Implement registration transaction pinning learning-path version and pre/post snapshot; existing attempt retrieval; lesson metadata/protected content/open/complete; answer autosave clearing/revision; submission; measurements/history and summary. Return safe DTOs, committed revisions/progress, no keys before reveal policy.
- [x] RED then GREEN concurrency/DB tests: stale saves, two simultaneous submissions, save vs submit, missing-answer submission, idempotency payload conflict/replay, DB rollback, question changes after pre-test, score null vs zero. Enforce school/enrollment ownership in every entry path. Preserve one final attempt/type initially.
- [x] BMI domain tests positive decimals, zero/negative/nonfinite/overprecision/storage overflow and stable one-decimal display; server recomputes and records rule version without clinical categories.
- [x] Explicit dev fixture/bootstrap support only if needed for complete test setup, with synthetic content clearly marked. No implicit production content import. Report seed shape for frontend tests.
- [x] Run full Rust gates with real DB tests, commit, self-review, report exact DTOs, tests and concerns. Parent obtains separate read-only review before Task 3.

## Task 3 — Admin content management and authorized reporting

**Ownership:** Backend feature worktree, learning/assessment admin application/HTTP adapters, reporting context, focused tests and docs.

- [x] RED: create/edit drafts, invalid publish, valid immutable question/bank/lesson/path publication, stale root revisions, order/archive, and active enrollment surviving later edits.
- [x] Implement every admin content/reporting endpoint in inventory; no catch-all mutable JSON API or hard deletion. Teacher terminology only when documenting legacy frontend input. Keep entity identities stable; question keys admin-only. Sanitize content or constrain rendering to safe plain text.
- [x] Report tests: granted vs ungranted school, filters applied before pagination, stable ties, full historical question/option/selected text, per-student and per-question views, missing/unanswered/incorrect/correct distinguished.
- [x] Complete `docs/runtime-contract.md` with real tested route/DTO/status examples for ALL frontend needs. List concrete dev bootstrap and API run commands, no passwords.
- [x] Run full Rust gates and fixture-based real HTTP workflow, commit and report; read-only review gate follows.

## Task 4 — Frontend integration and admin school UI

**Ownership:** Frontend isolated worktree only, existing `src/services/{api,http,contracts}.ts`, `src/domain/types.ts`, `src/hooks/`, relevant `src/pages/`, shells/router, tests, scripts/config/docs required to run actual Rust API. No backend edits; API discrepancies reported to parent rather than adding mismatched assumptions.

- [x] Consume reviewed `docs/runtime-contract.md` and running Rust endpoint examples. Inspect existing source/tests/visual conventions. Preserve legacy route migration and existing design; change user-facing teacher/guru to admin.
- [x] RED service/component tests for envelope/errors/204, cookies+CSRF, revisions, school picker and school add/edit/archive with confirmation, admin/session guards and retry/input preservation.
- [x] Replace browser-token storage and Node fixture integration with `/api/v1`, `credentials:'include'`, session/CSRF and stable IDs. Adapt canonical API DTOs to existing view models; fetch details on demand and paginate admin lists rather than downloading every student's private full state. Update CMS publication/revision UI and real reporting.
- [x] School login picker reads active directory; archive cannot remove historical result display. No frontend-generated progress/scores, no keys in student bundles.
- [x] Configure local Rust proxy/run modes without exposing services publicly or hardcoding credentials. Keep Node fixture API explicitly development-only and do not use it to prove integration.
- [x] Run `npm ci`, `npm test`, `npm run build`; add/run Playwright tests against real Rust/Postgres: admin provisions content/school, student completes flow with reload/resume, admin sees question/student results, archived school no longer selectable, role/locks enforced. Use synthetic isolated fixture data and no public credentials. Verify key responsive widths/keyboard flows for changed screens.
- [x] Commit, report exact commands/results and backend endpoint gaps, then parent obtains task review.

## Task 5 — Integration, final review and acceptance

**Ownership:** Parent coordinates isolated branches and runtime/test config, updates durable handoffs. Any source findings return to scoped implementers.

- [x] Apply reviewed migrations to local app database with migrator role; confirm DML-only app role cannot DDL. Start Rust on loopback 4180 and frontend on loopback 5173 with proper process ownership; preserve nginx/portfolio and foundation DB.
- [x] Exercise real HTTP cookies/CSRF, every learning stage and admin CMS against isolated synthetic integration data; browser reload restores DB state. Run all Rust, frontend, Playwright gates and verify source is not connected to Node fixture API.
- [x] Obtain fresh independent whole-change review across both repositories; fix material findings through reviewed scoped commits; report deferred limitations honestly.
- [x] Finalize `docs/handoffs/2026-11-15-full-stack.md` and corresponding frontend handoff with actual agent IDs/model metadata, commits, commands/results, running process handles and residual production gates.
- [x] Leave new feature branches/worktrees intact and do not push/merge until owner authorizes. Report what is working and how it was verified; no stub/plan-only completion.

## Execution ledger rules

Persist pre-flight task/interface checks and every scope/contract ruling under this plan's `.superpowers/sdd/` workspace, with task briefs/reports and immutable review packages. Model routing override was explicitly approved by owner in the clarification UI. Each backend task is sequential because shared bootstrap/transaction/DTO contracts must be inspected and reviewed first. Parent toolchain/database provisioning can proceed independently of source implementation. No concurrent implementers in one repository.

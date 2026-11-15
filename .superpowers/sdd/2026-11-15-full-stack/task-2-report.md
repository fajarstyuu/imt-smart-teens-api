# Task 2 implementation report

Status: ready_for_review (implementation complete; independent acceptance belongs to parent). BASE 23cd7c7f05c9fef766423c88c4644aada1530d8b. Available-model worker gpt-6.1-sol; effort unknown; no nested agents.

## Evidence
- Read task brief, accepted Task1 report/re-review and inspected session preparation/transaction insertion, CSRF, extractor, schema and router implementations.
- Initial RED: `cargo test --locked --test student_workflow -- --test-threads=1`: 0 passed, 2 failed, real migrated PostgreSQL. Both fail registration expected201 actual404 NOT_FOUND.
- Explicit test fixture creates two synthetic published lessons with required sections, paired and quiz banks each with single/multiple questions, immutable versions and a path. No production seed.

## Decisions / concerns
- Preserve reviewed cancellation, CORS and session foundations. All cross-context effects use a caller-owned transaction obtained through core repository ports.
- Enrollment replay scope is a separate signed HttpOnly browser-operation context established by GET csrf before enrollment. It remains stable across student/admin CSRF rotation, so both response-loss and received-cookie retry reuse the same scope. No raw session/csrf secrets cached in JSON. Context expiry bounds browser replay; no name-based or cross-device recovery.
- BMI uses checked fixed-point integer arithmetic at input scale4 and result scale12; no binary floating-point or silently rounded input. No dependency/driver decimal conversion needed.
- Existing blocking password-worker runtime teardown qualification remains; no universal hard-exit claim.

## Remaining
Implemented all inventory student routes and explicit domain/application/transaction repository ports, SQLx adapters and HTTP wiring. Runtime DTO document now exists at docs/runtime-contract.md. Initial full suite after first8 student tests: 37 passed; clippy all-targets/all-features -D warnings passed. Expanded student suite now11 passed.

## Additional actual RED/GREEN evidence
- BMI boundary test failed on exponent i32::MIN overflow from abs(); fixed by bounded comparison, then passed. Immutable historical test failed because pinned text UPDATE succeeded; append-only migration4 plus pinned-extension guards then passed. Fixture ordering was corrected to insert options before bank-item references, matching publication invariants.
- Full suite exposed CLI hardcoded migration count3 (actual4); changed to compare embedded MIGRATOR count, not silently skip. Existing persistence deletion check expected23503; immutable DELETE guard preserves that established constraint failure vocabulary. All29 existing tests passed afterward.
- Concurrent autosave test reached409 but latest safe revision was null/missing; neutral ConflictDetails and HTTP details mapping now return latest_answer. Clear/increment/cardinality/concurrent submissions/save-submit row-lock tests then passed.
- Registration normalization retry initially returned409 rather than201; normalize validated name before hashing. Numerically equivalent measurement retry60.50000/1.7e2 initially returned409; canonical fixed-point input hashing fixed it. Both observed RED then GREEN.
- Expanded verification: real PostgreSQL school archive lock race, audit-failure total enrollment rollback, missing-content atomicity, same-key simultaneous registration, coexisting admin/session rotation and foreign ownership. No production seed, application DB migration, host changes or nested agents.

## Final hardening and verification

- First scoped commit: f4da518 (`feat(student): persist transactional enrollment and learning journey`). Final hardening commit: adc339f (`fix(student): seal snapshots and align browser session lock order`). Both commits are scoped; post-commit status contains only the parent-owned handoff modification. `git diff --check 23cd7c7f05c9fef766423c88c4644aada1530d8b..HEAD` passed.
- Additional real RED: the quiz snapshot accepted a new snapshot question after enrollment while its quiz attempt did not yet exist. Migration5 seals snapshot construction to the creating transaction, covering both banks at enrollment commit. Test then GREEN.
- Additional real RED: malformed school UUID returned422 instead of inventory400. Explicit HTTP-only RegistrationRequest now maps malformed UUID syntax to400 before passing typed Registration into core. Test then GREEN.
- Additional real RED: a held admin-session row showed student rotation had already locked the student row, opposite reviewed admin-login order. New adapter now takes admin before student. A real PostgreSQL NOWAIT probe and blocked-query observation prove the corrected order; test GREEN.
- Self-review moved published-manifest availability decisions from a SQL boolean into domain PublishedPath::require_available; SQL supplies facts/counts only. Added method contracts and extracted explicit transactional synthetic fixtures into tests/fixtures/student.rs. Future publication fixture proves newer enrollment sees version2 text/options/path while existing pre/post and lesson bodies remain pinned to version1.
- Expanded full journey injects a real audit failure AFTER post scoring/enrollment completion; status, answer points, completion and key reservation roll back, then the same key succeeds after removing only the test-local trigger. Late new measurement rejects409; original measurement replay still201. Foreign history stays empty and foreign summary locked.

All final gates executed separately on the final code tree with PATH=/home/ubuntu/.cargo/bin:$PATH, CARGO_BUILD_JOBS=1, CARGO_PROFILE_DEV_DEBUG=0, CARGO_PROFILE_TEST_DEBUG=0. `.env.test` was privately sourced for DB commands, never printed:

1. `cargo fmt --all` then `cargo fmt --all --check`: exit0.
2. `cargo clippy --all-targets --all-features --locked -- -D warnings`: exit0, no warnings.
3. `cargo test --all-targets --locked -- --test-threads=1`: exit0, **44 passed, 0 failed, 0 ignored** (all29 foundation tests retained plus15 student tests). Suite totals computed in Python from actual output: CLI2, config1, domain4, foundation2, hardening2, HTTP3, persistence3, sessions3, student15, transactions3, transport6. SQLx-created PostgreSQL databases really ran.
4. `cargo build --locked`: exit0.
5. `python3 -m unittest discover -s tests/infra -p 'test_*.py' -v`: exit0, 3 passed.
6. `git diff --check`: exit0. Diff against BASE for the original three migrations is empty. Parent-owned handoff remains unstaged and untouched by this worker.

## Changed paths and public interfaces

- New domain/application modules: src/modules/{learning,assessment,anthropometry}/; identity application/presentation registration. SQLx student adapter is src/modules/learning/infrastructure/{enrollment,assessment,measurements,operations}.rs.
- Additive wiring only: src/modules/mod.rs, identity module exports, src/bootstrap/mod.rs, HTTP state/router/error. Shared neutral error adds ConflictDetails for safe stale-answer metadata. Existing CSRF/identity crypto implementation reused unchanged; GET csrf adds the stable browser-operation cookie in identity presentation.
- Append-only migrations4/5 implement published/pinned immutability and sealed snapshot construction. Original migrations1–3 untouched. Cargo.toml enables serde_json arbitrary_precision; no new dependency versions or Cargo.lock change needed. No SQLx numeric driver feature: checked integer decimal policy plus bound text::numeric conversion is exact.
- tests/student_workflow.rs + tests/fixtures/student.rs; tests/cli.rs compares migration count to embedded MIGRATOR so append-only additions retain CLI replay coverage. docs/runtime-contract.md is the exact authoritative implemented student DTO/route/retry contract.
- bootstrap::assemble(pool,settings) and build_router(AppState) signatures unchanged; AppState adds students: StudentService.
- StudentService::new(Arc<dyn StudentUnitOfWork>, SessionService). Explicit methods: register, me, progress, lesson, open_lesson, complete_lesson, create_attempt, attempt, save_answer, submit, measure, measurements, summary. Register returns safe Value + IssuedSession; HTTP alone writes cookies after commit.
- StudentUnitOfWork::begin -> Box<dyn StudentTransaction>. That one transaction exposes IdentityRepository, LearningRepository, AssessmentRepository, MeasurementRepository and OperationRepository supertrait ports and owns commit. No independent cross-context commit. Dropping any failed/cancelled use case rolls back.
- Core IDs: StudentScope {student_id,enrollment_id}; AssessmentType::{PreTest,Quiz,PostTest} serializes pre_test|quiz|post_test. Registration {name,school_id,form_date}; RegistrationContext carries operation, new CSRF binding and only hashed prior/sibling session handles. Operation validates printable ASCII key, fingerprints validated input and persists safe results.
- Exact payloads/envelopes/errors are enumerated in docs/runtime-contract.md, including latest_answer revision conflicts, nullable score visibility, question snapshot IDs, measurement JSON numbers and one-decimal display, bodyless204, and pagination.

## Task3 setup and concerns

- Use tests/fixtures/student.rs::content for synthetic published version1 setup and ::publish_future for version2 examples; neither is linked from runtime bootstrap. Task3 must publish complete immutable children before referencing manifests, in one transaction, and insert snapshot questions in the creating snapshot transaction. Draft roots can change; never UPDATE/DELETE published rows. Validate bank keys/cardinality/rules and complete path publication before exposing it to new registrations.
- Browser frontend must keep credentials included, operation keys across retries and returned csrf_token in memory. Preserve the separate enrollment-context cookie; no bearer token/Node fixture aliases. Replay is bounded by possession of its unexpired two-hour signed browser context, while cache TTL is24h. No automatic uncertain-enrollment retry with a new context and no name/cross-device recovery.
- No public/development production seed, frontend edits, application DB migration/data changes, host/nginx changes, Docker recreation/volume deletion, push or merge. No required gate skipped. No detached/blocking workers added.
- Carry the accepted Task1 qualification: existing spawn_blocking password work may delay ordinary Tokio runtime teardown; no universal hard process-exit guarantee is claimed.
- Independent read-only review is pending and belongs to parent. This is not final whole-app acceptance.

## Fix round1 incremental evidence

- Recovery starts at FIX_BASE b481591dbe2e0d8a9173b4c55c239ad70044d6a3; parent-owned handoff left untouched.
- Real RED: `cargo test --locked --test student_workflow task2_regressions -- --test-threads=1`: 1 passed, 4 failed. Paired maximum+maximum, quiz maximum+maximum and paired maximum+one-unit were incorrectly admitted201 instead of409. Earlier-begun completion was deterministically held after real PostgreSQL BEGIN using oneshot barriers until a later opening/visit committed; completion returned500 instead of200. Boundary submission already passed and is retained as a guard.
- First GREEN with shared checked bank budget at admission and before score writes, plus completion write-time clock bounded by opened_at: all5 regressions passed. No migration changes. Fixtures assign weights before immutable publication references seal them; failed paired enrollment keys successfully retry after a new valid publication.
- Typed core/cache/DTO correction and final full gates remain in progress; this is not acceptance.

# IMT Smart Teens — final local full-stack handoff

Date: 2026-11-16 (runtime UTC+08:00).
Status: LOCAL IMPLEMENTATION ACCEPTED WITH EXPLICIT MINOR BACKLOG. Integration into main, remote push and public rollout await owner authorization. This is not production approval.

## Delivered and exercised

- Rust/Axum modular monolith, PostgreSQL/SQLx persistence, 28 business tables and five append-only migrations. Domain/application contracts are typed and infrastructure-independent.
- Separate student/admin cookie sessions, Origin/CSRF protection, role permissions and school-specific reporting grants. Product role is admin; remaining Teacher* symbols are legacy frontend filenames, not a teacher role.
- Atomic enrollment and pinned immutable paired pre/post and separate quiz snapshots; persisted pre-test, sequential lessons/required visits, quiz, BMI measurements, post-test and historical summaries. The server owns progress, grading and measurement results.
- Admin lesson/question/bank/path draft, revision, publication, order and archive workflows; school add/edit/archive; authorized historical enrollment and question-version reports. School deletion is archive, preserving historical identity and results.
- React integration uses real v1 HTTP/cookies, not the historical Node fixture server. Failed/uncertain operations retain scoped identity and require deliberate reconciliation; there is no automatic uncertain enrollment replay or durable client answer queue.

## Accepted source and review chain

- Backend branch: feat/full-backend; runtime source d505fd6336e2c570c547a418484cefcedebedfac, reviewed documentation checkpoint febf49767d55c6f23a1f618201b5ce1b5e17cfc6. Later changes in this closure are documentation only.
- Frontend branch: feat/rust-api-integration; accepted runtime/build source b3c52b89e04e3be99b630a5f37e4634f28fca5e2. Final correction commits: e951631, 5e96bff, b3c52b8.
- Local main fork points remain backend d3327ae49ed7dd153662ec29d432fb78470c9f43 and frontend 6344f0b7304d9cfcd5b642034ccf33b2c3393585. No feature merge or push occurred.
- Whole-change reviewer sa-0-552392be / deleg_f62269b1 found 0 Critical, 3 Important, 7 Minor. The parent accepted all three Important findings and assigned one consolidated fix wave sa-0-5795d001 / deleg_51d45296.
- One final scoped reviewer sa-0-99c74eec / deleg_d29505ae inspected the complete correction, relevant source and actual RED/focused GREEN evidence. I1/I2/I3: ADDRESSED; scoped spec PASS, quality ACCEPTABLE, no new breakage identified. The parent accepts the local milestone with the seven Minors below, not an unqualified production/spec certification.
- Orchestrator: Mochi, gpt-6.1-sol. Child routing used available inherited models; per-child exact model/effort was not exposed. No Luna/high execution is claimed.

### Final finding closure

1. I1: mounted-owner guards protect failed/in-flight answers across menu, history, unload and target-role logout. Cancellation preserves selection/error/revision. Real Rust commits an answer whose acknowledgement is lost; deliberate retry observes actual409, then saves revision2, with persisted readback after return/reload. Only the acknowledged/discarded owner is released.
2. I2: refreshing a lesson revision acknowledges the current server baseline without replacing local input. Any divergence, including previously clean old input, blocks publication until deliberate resolution/save. Real clean/dirty two-editor scenarios publish the reviewed visible text/sections and compare immutable student readback.
3. I3: active Rust tests cover both lesson/question editors, fragments, repeated/multistep Back, cancellation/discard, role logout and accepted Back/Forward. These tests exposed an early POP ordering race; the dispatcher now runs before Router unmounts a dirty owner. No global guard reset or credential persistence was introduced.

## Final verification

The parent freshly executed the following on the accepted source before this documentation closure:

- Rust: 65 tests passed, 0 failed/ignored; three infrastructure tests passed. Formatting, all-target/all-feature warnings-denied Clippy, locked build and branch-range whitespace checks passed.
- Frontend: 40 Vitest tests across 14 files and six explicitly labeled legacy-domain tests passed. TypeScript and Vite/publisher build passed. The legacy six are not Rust integration evidence.
- Real browser: nine serial Playwright cases against real Rust/PostgreSQL passed. Explicit output corroborates three canceled answer navigations, persisted answer revision2, two reviewed published drafts, matching pinned text/sections, exactly one enrollment POST after committed projection failure, and ten navigation/logout prompts for each editor with correct session/Back/Forward outcomes.
- Separate native Chrome zoom: one case passed; browser zoom factor2, layout width720, DPR2, visual scale1, four exercised screens and reset1. This is native browser zoom, not CSS or viewport emulation.
- Implementer also recorded clean install with zero vulnerabilities, built-SPA/assets smoke, matching root/dist entries, and private-literal scans. Two existing React Router module-level use-client warnings remain disclosed.
- At 2026-11-16T16:00:52+08:00 the restored built frontend and Rust readiness both returned200.

Backend gate (private variables are sourced, never printed):

```sh
export PATH="$HOME/.cargo/bin:$PATH" CARGO_BUILD_JOBS=1
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
set -a; . ./.env.test; set +a
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked -- --test-threads=1
cargo build --locked
python3 -m unittest discover -s tests/infra -p 'test_*.py' -v
```

Frontend gates: `npm test`, `npm run build`, `npm run test:e2e`, then separately `npm run test:e2e:zoom`. Supply operator-owned private synthetic E2E configuration and the school-grant helper as described in the frontend README. Minimal-host browser libraries were scoped through LD_LIBRARY_PATH, not a new privileged host install.

Both browser configurations deliberately require ownership of port5173 (`reuseExistingServer:false`). Stop only the identified parent preview before running them; let each runner own/clean its dev server, then restore `npm start`. The parent's first final browser invocation correctly refused the occupied preview URL before running any tests; after stopping that identified preview, all nine plus native zoom passed without source/config changes. Never weaken this ownership check or rate limits to obtain green tests.

## Current local runtime and database state

- Backend worktree: /home/ubuntu/imt-smart-teens/api/.worktrees/full-backend.
- Frontend worktree: /home/ubuntu/imt-smart-teens/web/.worktrees/rust-api-integration.
- Rust loopback4180: parent process proc_a9cb8e82081a, PID344445. It intentionally uses the isolated synthetic frontend integration database, not the original application database.
- Built frontend loopback5173: restored parent process proc_f3fc626cb02c, PID1056161, Vite PID1056191. Prior preview handles are historical. Process IDs/handles are observations, not permanent restart instructions.
- Existing PostgreSQL loopback55432 and its foundation Compose worktree/volume are retained. Nginx/portfolio routing was not changed by this full-stack milestone.
- Original local application database imt_smart_teens_dev was verified empty before applying migrations202611150001..5: 28 business tables, two empty bank roots and one path draft; zero admins, students or published paths at setup verification. Runtime role owns no tables and has no migrator membership or schema CREATE; actual CREATE/ALTER probes were denied and rolled back.
- No real operator account, reporting grant or teaching content was invented in that application database. Provision these privately and deliberately before using it. Development-only init-content-config creates missing empty roots, not demo lessons or production credentials. See docs/runtime-contract.md for actual migrate/init/provision/serve contracts.
- Existing private backend .env/.env.test and synthetic test configuration are not committed or attached. All credentials/authentication artifacts/connection strings are [REDACTED]. Scratch E2E configuration is not a production deployment configuration. Do not blindly restart the synthetic API using the original application .env or point browser fixtures at application data.

## Deferred minors — exhaustive, not silently fixed

1. M1: CMS detail metadata uses multiple READ COMMITTED queries and can mix revisions under concurrent writes. Writes/publication still enforce revision/ownership/locks. Backlog: coherent detail-read snapshot plus concurrency coverage.
2. M2: unfinished dashboard fallback completion copy can mislead; server progress/gates remain correct. Backlog: separate unfinished copy without fetching locked bodies.
3. M3: some CMS wire enums are shown untranslated. Backlog: Indonesian presentation labels without changing wire identifiers.
4. M4: Compose test covers empty passwords but lacks a separate unset-password regression. Required interpolation already rejects unset values. Backlog: isolated absence case.
5. M5: wrong-password smoke classifies any nonzero result rather than an authentication-specific sanitized diagnostic. Positive control exists. Backlog: precise rejection classification.
6. M6: two existing React Router use-client bundling warnings. Build succeeds. Backlog: justified upstream/toolchain resolution, not blanket suppression or speculative upgrades.
7. M7: report-only admins must enter a historical question-version UUID manually. Scoped report access exists; content grants must not be broadened. Backlog: link authorized historical answers/versions directly into scoped reports.

## Rulings I made — exhaustive decisions and costs

These are milestone decisions, not a claim that deferred Minors were fixed. Repeated final compatibility qualifications are retained because the final reviewer explicitly left them to the owner.

1. Use owner-approved available-model subagents and sequential dependent task reviews; pinning exact model/effort is unavailable. Cost if wrong: revise attribution/review policy, never invent Luna metadata.
2. Preserve the foundation worktree because its private environment and Compose project own the running database. Cost if wrong: retain an old local worktree until an explicit deployment move.
3. Map conceptual backend/ paths to the separate API repository root. Cost if wrong: path-only refactoring, not a duplicated nested service.
4. Provision limited runtime/test roles and an isolated test database, with private environments. Cost if wrong: stop and adjust grants before application migration; no existing credential rotation or data deletion.
5. Recover and checkpoint tested source after VPS worker shutdown rather than repeat implementation. Cost if wrong: scoped review corrections, without duplicate side effects or unverified completion.
6. Treat domain-to-application error imports as an Important inward-dependency defect. Cost if wrong: a small compatibility-preserving error-module refactor and complete tests.
7. Keep NUMERIC(12,4) aggregate storage and atomically reject unrepresentable bank totals before enrollment/publication. Cost if wrong: exceptionally weighted banks need an explicit wider-score migration; never clip or enroll into an unfinishable attempt.
8. Use explicit private idempotent development-only initialization of empty content roots. Cost if wrong: production initialization needs its own authorized policy/operation; no automatic content or credentials.
9. Require immutable question_version_id for cross-student question reporting and distinguish pending selection from graded incorrect. Cost if wrong: the frontend must supply historical version selection.
10. Resolve revision/locking/order/draft-to-immutable-ID policies in the typed wire contract, retaining the equal max_points1 editor contract. Cost if wrong: documented contract correction and scoped review, not assumed compatibility.
11. Prepare a new isolated synthetic PostgreSQL integration database before application migration. Cost if wrong: separate final application migration/production governance remains necessary; no browser fixtures in application data.
12. Block unresolved enrollment instead of automatically renewing CSRF/context and replaying. Cost if wrong: conservative blocked-retry UX; authenticated complete recovery is allowed without a second POST.
13. Leave real-data privacy/retention/child-consent/school-grant governance to an owner production gate, since only synthetic local tests were authorized. Cost if wrong: rollout waits for policy/operator review and necessary remediation; no certification claim.
14. Leave public TLS/trusted-proxy/release/backups/scaled capacity to separately authorized rollout. Cost if wrong: further configuration/design/verification; never trust arbitrary forwarded headers or weaken rate limits.
15. Accept fail-closed CacheV1 for the verified empty predeployment install, not seamless live-cache upgrades. Cost if wrong: future compatibility/drain/backfill design; no blind deletion/replay.
16. Limit shutdown acceptance to tested HTTP/SQL/pool cleanup, not universal hard process exit during arbitrary blocking Argon2 work. Cost if wrong: stronger deadlines need runtime/supervisor policy and isolated proof.
17. Treat responsive/keyboard/native zoom as limited UI evidence, not pixel-perfect/screen-reader/WCAG certification. Cost if wrong: a dedicated audit/remediation before certification claims.

## Owner integration decision

The local implementation/verification milestone is accepted; the seven Minors and five production qualifications remain explicit. Preserve both branches/worktrees and existing private operational files. No feature merge, push, PR, public deployment, host reconfiguration or data destruction was performed. The next owner choice is: merge both branches into local main, push/create PRs, or keep both branches as-is. Any selected integration requires checking the actual target and re-verifying the resulting tree.

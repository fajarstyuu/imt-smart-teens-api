# Final independent production-deployment review

## Assessment

**Findings: 0 Critical, 1 Important, 0 new Minor.**

**Spec acceptance: With fixes. Quality / ready-to-integrate verdict: With fixes.** The deployed topology, frontend integration and exact-peer security boundary match the approved design, with substantial attributed execution evidence. The root-run secret-preparation helper has one material operator-safety defect; address it in the authorized bounded correction pass before final source acceptance. This finding is not evidence that the current approved production secret path is damaged or that the running service must be rolled back.

Reviewed backend `3957fc16aa4ada5ed1bdb4159028dcd4374c0103..518ad4b1b731f7efcb5b2e84ed3ab19f493ed57c` and frontend `d985ff95ab925229ef1f19516c81b471aa12b38f..66719b15315e83efa736ade699951fc06386c385`. B means `/home/ubuntu/imt-smart-teens/api/.worktrees/production-deployment`; F means `/home/ubuntu/imt-smart-teens/web/.worktrees/production-deployment`. This is the single whole-change review, not a new whole-application audit.

## Strengths

- Small, explicit Rust identity resolver: exact socket-peer allowlist; only one parseable X-Real-IP; malformed/duplicate/missing values fall back to the peer. No arbitrary forwarded-chain trust, no limiter-budget reduction, and no Origin/cookie changes hidden in the identity patch.
- Compose limits published ingress to loopback frontend; database and backend stay private. Separate runtime/migrator secrets and role grants, explicit migrations/root initialization, persistent named storage, pinned base references, non-root/read-only application containers and bounded logs/resources are coherent across both repositories.
- Host templates and the two installed IMT site files agree on IPv4/IPv6 listeners, TLS paths, IP-header overwrite, preserved Origin and API-host application denial. The ACME exception serves files, not an upstream. Renewal reload is scoped to the IMT lineage and validates Nginx first.
- The guide treats restoration, production provisioning, retained worktree bind mounts and rollback as operator concerns rather than inventing production accounts/content or claiming that static checks prove runtime behavior.

## Issues

### Critical

None found in this scope.

### Important — I1: Secret preparation can change shared system-directory ownership and permissions before validating its target

**Location:** B `deploy/prepare-secrets.py:21-32`, particularly lines 24-26; reached with root privileges through B `deploy/operations.sh:14-18` (`prepare`).

**Evidence:** The helper resolves the configured secret directory and rejects only `/` itself or a final-component symlink. It then unconditionally calls `base.parent.mkdir(..., exist_ok=True)`, `base.parent.chmod(0o700)` and `os.chown(base.parent, operator_uid, operator_gid)`. Only afterward does it check whether the requested directory already contains a valid secret set. There is no restriction to the approved production/smoke private parent or verification that an existing parent is a dedicated IMT directory.

**Trigger and effect:** A misplaced but syntactically accepted operator setting such as `IMT_SECRET_DIR=/secrets` selects `/` as the parent; `/etc/imt-smart-teens` selects `/etc`. `operations.sh prepare`, run as the documented non-root operator through sudo, would chmod that shared parent to 700 and transfer ownership to the operator. This can deny other users/services directory traversal and give the operator ownership of shared system paths, damaging unrelated services including the portfolio. An existing invalid secret target is not safe either: its eventual rejection happens after these parent mutations. This is a configuration-error safety problem in a privileged helper, not a claimed remote attack or an observed incident on the currently deployed path.

**Required correction:** Validate the complete target and parent boundary before any mutation. Constrain preparation to an explicitly approved dedicated private parent (with a deliberate, validated extension mechanism if relocation is required); reject shared/root/unsafe resolved parents and inappropriate symlink ancestry. Do not chmod/chown an arbitrary pre-existing parent. For an existing target, validate and preserve it without changing unrelated directories. Retain the existing atomic fresh-set publication and refusal to rotate partial sets.

**Bounded regression proof:** Cover `/secrets`, `/etc/imt-smart-teens`, a partial/unknown existing target and a symlinked unsafe ancestry with mocked filesystem mutators or an isolated owned test tree. Assert rejection with zero parent chmod/chown calls and no newly published secrets; retain fresh dedicated-directory setup and idempotent existing-set preservation checks. No production filesystem probe is necessary.

**Evidence limitation:** This finding follows directly from the inspected control flow. An optional in-memory mocked confirmation was submitted but the tool blocked it pending approval and reported zero tool calls executed; it was not rerun or represented as a successful test. No dangerous path was actually prepared.

### Minor

No new Minor findings raised. The seven previously deferred Minors are not reopened.

## Five required review-focus dispositions

1. **Forged or duplicated client-IP headers cannot bypass throttling; distinct legitimate visitors behind the trusted frontend must not share one backend bucket.** Source alignment: B `src/transport/http/client_ip.rs:5-21`, middleware `:89-105`, host FE template `:30-35`, F `deploy/nginx.conf:12-25`. Host edge replaces public IP headers, frontend forwards the sanitized value, backend trusts only the fixed frontend peer. Proxy-policy tests explicitly cover duplicate/malformed/default-untrusted behavior and separate IPv4/IPv6 identities; parent reports actual frontend-socket and public forged-header probes. No additional material defect found. Local privileged/host-network actors are not an untrusted public header boundary.

2. **Recreating containers cannot reset secrets/roles or lose committed production data; an existing nonempty database must not be silently reinitialized.** Named volume, fresh-volume-only role initialization, separate explicit operations and conflict-preserving transactional root SQL align. Parent reports actual recreation persistence and private backup/restore. No reset/delete-volume operation is supplied. I1 is the remaining privileged preparation failure case; it does not negate the supplied recreation evidence or claim current data loss.

3. **Proxy Host/Origin/cookie handling cannot turn a foreign-origin mutation into an allowed request or widen cookie scope.** Neither proxy rewrites Origin or cookie Domain. B middleware `:118-122` invokes existing CSRF checks; B `src/transport/http/csrf.rs:65-80,107-112` compares the exact configured Origin/token and emits host-only Secure cookies in this configuration. Parent's actual foreign-Origin/cookie evidence supports the source boundary. No bypass found.

4. **Unknown hosts, API subdomain paths, IPv6 listeners and ACME exceptions cannot expose backend/admin/health ports outside the frontend policy.** Both installed IMT files match their reviewed policies; API has no proxy_pass; backend/PG have no published ports; frontend only proxies `/api/v1/`, not backend `/health/`. Read the enabled-site link names and existing default site: its HTTP default returns404 and its portfolio route is static, not a backend upstream. Do not interpret this as an assertion that every possible SNI/Host mismatch is explicitly rejected: Nginx selection may retain an already selected frontend vhost, but the resulting route still has the same public-frontend API/auth/Origin boundaries, not a new backend/admin/health bypass. Parent verified API denial and IPv6 localhost SNI; no independent off-host/IPv6 coverage claimed.

5. **Container/DNS/certificate/renewal failures must leave the existing portfolio valid and support rollback without deleting any volume.** Separate site targets, validation-before-reload, scoped renewal hook and retained-data rollback align. Parent reports unchanged portfolio config/body and successful real renewal simulation. I1 violates the broader operator-failure isolation expectation because secret preparation can mutate shared filesystem parents independently of those careful host-routing steps.

## Evidence ownership and limits

**My own reads/checks:** Read the complete immutable B diff (1,560 lines) and F diff (269 lines) in bounded passes; read the spec, plan, handoff, operator guide, native ledger and review template. Followed only relevant middleware/CSRF/bootstrap/schema sources and named host site configurations. Confirmed both exact HEADs. Read the raw final backend-gates log, including the four proxy-policy successes and infrastructure summary. Ran read-only `git diff --check` across both exact ranges and `git status --porcelain`; exit0 with empty output. No source/index/ref changes were made.

**Attributed parent evidence, not new reviewer execution:** 69 Rust tests/0 failures/0 ignored; fmt, warnings-denied Clippy and locked build; 11 infrastructure tests; frontend 44 Vitest plus 6 legacy tests/build with two disclosed pre-existing Router warnings; actual pinned image builds; isolated role-denial/migration/persistence/backup-restore checks; public TLS/DNS/routes/cookies/Origin/body-limit/header-spoof checks; successful scoped renewal dry-run/timer; anonymous production Chromium and visual inspection. Raw backend-gates output corroborates the supplied backend/infra account, but it is not a rerun by this reviewer. The shared Cargo target paths in that log do not independently identify the invoking source directory; exact-tree attribution comes from the parent handoff/context.

No broad tests, builds, browsers, public script, DB operations, Certbot, containers, daemons or host mutations were run. No secret files, authentication artifacts or profiles were read. The attempted optional mocked probe remains unexecuted pending approval; no retry is required for the source-based finding.

## Declined to judge / questions

- Production enrollment/content readiness: deliberately empty production dataset; real administrator/school/content/grants provisioning is a separate authorized owner operation, not a deployment defect.
- Authenticated production learning/editor flows: previous synthetic-environment evidence is not promoted to production proof; creating fixtures/authentication artifacts here is prohibited.
- Privacy/retention/child-data governance, full accessibility/pixel-perfect behavior, capacity certification and multi-replica limiter design: explicit milestone exclusions; no certification inferred.
- Independent off-host/cloud-firewall/public IPv6 availability: supplied checks originate on the VPS and there is no advertised AAAA; this review cannot turn them into independent external coverage.
- Existing unrelated application code and seven deferred Minors: not reopened by this deployment review.
- Unspecified recovery from an already lost secret set or incompatible future schema release: requires a separately authorized recovery/release decision; preserving existing secrets, backups and volumes is the current documented boundary, not an automatic destructive repair promise.

No additional question blocks the bounded correction. Parent should disposition I1 by its actual operator effect. Do not rerun the broad deployment acceptance suite merely to address it; use the targeted non-production preparation regressions and a scoped review of the correction.

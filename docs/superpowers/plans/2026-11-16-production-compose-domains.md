# Production Compose and Domain Deployment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (recommended here) or superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Completion:** Locally deployed and parent-accepted after the single independent review (one Important operator-path finding) and one RED/GREEN correction.69Rust/16infra/44Vitest+6legacy/build gates passed; real image/container/persistence/restore/public browser/TLS/renewal checks passed. See final deployment handoff for qualifications; new branches remain unmerged/unpushed.

**Goal:** Build and run the frontend, backend and isolated production PostgreSQL in Compose, with valid domain TLS and frontend-only proxied API access.

**Architecture:** Host Nginx terminates TLS and forwards only the frontend domain to loopback5180. Container Nginx serves the Vite build and forwards /api/v1 to internal Rust4180; PostgreSQL has no published port. The API hostname denies public application access. Explicit private operations initialize the database; normal serving never seeds accounts/content.

**Tech Stack:** Existing Rust1.100.0/Axum/SQLx/PostgreSQL17, React/Vite, Docker Compose, Nginx and Certbot.

**Spec:** docs/superpowers/specs/2026-11-16-production-compose-domains-design.md (owner-approved chat architecture; builder decisions identified there).

## Global Constraints

- Preserve all current services, databases, volumes, portfolio routing and credentials. No production down -v, account defaults, fixture copying or secret output.
- Exact domains: imtsmartteens.my.id and api.imtsmartteens.my.id. Exact backend allowed origin: https://imtsmartteens.my.id. Secure cookies; no Origin rewriting.
- New production Compose project imt-production. Only127.0.0.1:5180 is published; backend/database remain internal. App subnet172.30.251.0/28, frontend.2/backend.3 after deployment-time conflict check; separate internal DB network.
- Host Nginx overwrites IP headers. Backend trusts only configured exact peer IPs, default none. Preserve10login/300API per-IP per60seconds; never globally disable the limiter.
- Three long-lived services plus explicit operations profile. Runtime database role cannot DDL or inherit migrator privileges. No automatic schema/content initialization during serve.
- Secrets outside repositories/build contexts, private directory700/files400 with verified container UID ownership. Separate runtime/migrator/bootstrap mounts; all secret/auth values [REDACTED].
- Official base images resolved/pinned by digest before implementation; locked dependencies; serial single-job Rust builds/debug0 on limited memory.
- Code/comments/docs English, UI Indonesian. No new product features or unrelated deferred-Minor fixes. No automatic new-branch merge/push.

## Review Focus

1. Forged or duplicated client-IP headers cannot bypass throttling; distinct legitimate visitors behind the trusted frontend must not share one backend bucket.
2. Recreating containers cannot reset secrets/roles or lose committed production data; an existing nonempty database must not be silently reinitialized.
3. Proxy Host/Origin/cookie handling cannot turn a foreign-origin mutation into an allowed request or widen cookie scope.
4. Unknown hosts, API subdomain paths, IPv6 listeners and ACME exceptions cannot expose backend/admin/health ports outside the frontend policy.
5. Container/DNS/certificate/renewal failures must leave the existing portfolio valid and support rollback without deleting any volume.

## Task 1 — Backend trusted-proxy identity

**Files:** Modify src/config/mod.rs, src/transport/http/middleware.rs; create src/transport/http/client_ip.rs and tests/proxy_policy.rs; update docs/runtime-contract.md and .env.example.

**Interfaces:** HttpSettings gains trusted_proxy_ips: Vec<IpAddr>, from optional TRUSTED_PROXY_IPS comma-separated exact addresses. client_ip::resolve(peer: IpAddr, headers: &HeaderMap, trusted: &[IpAddr]) -> IpAddr returns a single valid X-Real-IP only when peer is trusted; absent/malformed/duplicate values return peer. No CIDR/wildcard acceptance and no X-Forwarded-For/Forwarded trust.

- [x] Write focused RED tests for default-untrusted behavior, invalid config, spoofed headers, duplicate/comma/non-IP X-Real-IP, IPv4/IPv6, trusted peer and per-visitor limiter separation. Run cargo test --test proxy_policy --locked -- --test-threads=1; expected meaningful missing behavior failures.
- [x] Implement typed parsing/resolver and use it in policy without changing CSRF/CORS ordering or limiter budgets. Existing callers with no allowlist retain peer behavior.
- [x] Run focused GREEN plus fmt, warnings-denied all-target Clippy, complete real-Postgres cargo suite and locked build using existing private test environment, single build job. Expected0failures/ignored and unchanged old behavior.
- [x] Commit scoped code/tests/docs and ledger RED/GREEN results. This task produces the exact trusted-proxy contract consumed by Task2.

## Task 2 — Real images, Compose and private operations

**Backend files:** Create Dockerfile, .dockerignore, deploy/compose.production.yaml, deploy/.env.production.example, deploy/backend-entrypoint.sh, deploy/postgres/init-roles.sh, deploy/postgres/initialize-empty-content.sql, deploy/operations.sh, tests/infra/test_production_contract.py and docs/deployment.md.

**Frontend files:** Create Dockerfile, .dockerignore, deploy/nginx.conf and tests/production-proxy.test.ts; modify vite.config.ts and README.md only for deployment/dev-proxy contract.

**Interfaces:** Compose reads nonsecret BACKEND/FRONTEND build context paths and secret directory from its operator env. Backend context defaults to API checkout; FRONTEND_BUILD_CONTEXT must explicitly name the frontend checkout, so isolated worktree paths are never guessed. Production frontend listens8080, proxies /api/v1 to backend:4180 and provides local /healthz. Backend's sole trusted proxy is172.30.251.2. Private entrypoint maps explicitly mounted *_FILE secrets into existing CLI environment names without echo; exec preserves signals. Operations expose prepare, migrate, initialize-empty, up, status and backup subcommands; no credential CLI arguments.

- [x] Write RED packaging tests: only frontend publishes127.0.0.1:5180; no published postgres/backend; production/secure/exact-origin configuration; separated secrets/networks/roles; health dependencies; no default credentials; Docker build exclusions; locked multi-stage/non-root images; no startup seeding. Frontend tests assert path/Origin/cookie/IP-header preservation, static deep-link fallback and no Vite preview production command. Run targeted unittest/Vitest, expected failures for missing artifacts.
- [x] Resolve official image manifests/digests and write pinned multi-stage images. Retain Rust toolchain1.100.0 and a compatible official Node image, frontend Nginx unprivileged and PostgreSQL17. Verify actual image users before secret ownership setup. Make runtime filesystems read-only with only required tmpfs; drop application capabilities; add no-new-privileges, restart policies, bounded logs and healthchecks.
- [x] Implement explicit bootstrap: generate fresh private secrets once (never regenerate existing files); PostgreSQL init creates limited migrator/app roles and schema/default DML grants safely quoted. Migrate via existing private migrate CLI. Initialize only missing paired/quiz roots and one path draft using current schema/locking invariants in an explicit authorized SQL operation, never development-mode bypass. Refuse ambiguous existing root configurations. No admin/school/content creation.
- [x] Preserve Vite development proxy with changeOrigin:false and explicit configurable backend target; production frontend uses container Nginx instead. Forward only sanitized X-Real-IP from loopback host edge; never transform an untrusted Origin into the allowed origin.
- [x] Run targeted GREEN, complete frontend tests/build and infrastructure tests. Build images serially. Start an independently named smoke project/volume, run migrations twice, verify DML-only runtime CREATE/ALTER denial and unchanged root counts, secure CSRF cookies, exact allowed CORS, rejected foreign-origin mutation, header spoofing isolation and real container health.
- [x] In the owned smoke database only, create a clearly synthetic persistence probe, recreate service containers without deleting volume, and verify the record survives. Do not use the production database for test users/content. Clean only explicitly owned smoke resources or preserve them if ownership is uncertain.
- [x] Commit scoped artifacts with real build/container evidence and private-literal scan. Task2 produces deployable images and exact loopback ingress5180 for Task3.

## Task 3 — Production start, host TLS and public verification

**Files:** Create deploy/host-nginx/imtsmartteens.conf.template, deploy/host-nginx/api-imtsmartteens.conf.template and docs/handoffs/2026-11-16-production-deployment.md. Host targets: only /etc/nginx/sites-available/imtsmartteens and api-imtsmartteens, corresponding symlinks, and dedicated ACME webroot. Never overwrite the portfolio site or shared TLS settings.

- [x] Check live DNS A/AAAA, public IP, ports, subnet conflicts, Nginx config and existing Certbot account; save protected backups of exact changed host files. Expected both new names point here; do not attempt issuance if they do not.
- [x] Prepare production secrets/new volume, run explicit migration/empty-root operations and start imt-production. Verify container health, role restrictions, unpublished backend/Postgres, production environment and preserved old services before public routing.
- [x] Stage HTTP ACME vhosts, run nginx -t, reload only on success. Keep API deny inside application location so ACME challenge is reachable. Verify challenge path using the actual public hostnames before requesting one SAN certificate for both names through the existing Certbot account. If account/terms or DNS authorization is missing, stop and ask; do not invent credentials.
- [x] Install TLS vhosts: FE forwards to127.0.0.1:5180, overwrites Host/IP forwarding headers appropriately, preserves Origin, and provides HTTP-to-HTTPS redirect. API application paths return403 with no backend upstream; challenge exemption only. Preserve the portfolio and test nginx -t before reload. Ensure existing renewal mechanism covers the new certificate; verify renewal dry-run when permitted by the existing account/challenge setup.
- [x] Verify real HTTPS with CA validation and exact certificate SANs; frontend assets/deep links/proxied config/CSRF cookies; allowed FE Origin and denied foreign-origin mutations; direct API root/api/admin/health deny; both IPv4/IPv6 policy where DNS/listeners apply; forwarded-header spoof resistance and separate visitor rate buckets in controlled smoke requests. Prove rizkyfs.my.id still serves correctly. Never log cookie/token bodies.
- [x] Record rollback: restore only the backed-up IMT vhost targets and reload after nginx -t; stop only the new stack if necessary; retain named data/secret files. Document empty production admin/school/content state and operator setup requirements, not fictional readiness.

## Final verification/review and handoff

- [x] Run complete backend/frontend/package gates on exact source, actual images/Compose validation/container smoke, and public TLS/routing checks. Attribute local E2E results accurately: existing synthetic localhost browser tests are not proof of production data readiness.
- [x] Obtain ONE independent whole-change review covering both repositories, live evidence, five Review Focus classes and security/rollback/secret boundaries. Regrade findings by user effect. One consolidated Critical/Important correction pass with RED/GREEN regression proof; seven old Minors remain outside this milestone. Any required scoped confirmation stays bounded rather than repeating broad audits.
- [x] Commit final deployment handoffs, record exact image digests/branches/commits/commands/results/production process ownership, endpoints and unresolved limitations. Secret values remain [REDACTED]. Preserve runtime worktrees/private files.
- [x] Report working public behavior and ask owner before new branch merges/pushes. Do not claim success if DNS, TLS issuance, registry access, build or real container execution is blocked.

## Plan self-review

Approved scope is covered by Tasks1–3 and final acceptance. Task1's exact proxy-IP interface feeds Task2; Task2's loopback5180 ingress feeds Task3. All five Review Focus classes have explicit tests/checks in their owning tasks. New bootstrap is a private operator SQL step, not a weakening of the existing development-only CLI. Main and existing feature checkouts/services remain untouched by implementation; create isolated deployment worktrees before code changes. This plan is not permission to copy real data, create real operator credentials, publish secrets or integrate unreviewed source.

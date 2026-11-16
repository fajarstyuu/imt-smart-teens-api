# Production Compose/domain deployment — execution handoff

Date: 2026-11-16, runtime UTC+08:00.
Status: accepted by parent after one independent whole-change review and one tested material correction. Live Compose/domain/TLS delivery is complete; the deployment branches are not merged/pushed. Production account/school/content provisioning remains a separate owner operation.

## Scope, source and authorization

The owner requested Compose frontend/backend/PostgreSQL and domains imtsmartteens.my.id/api.imtsmartteens.my.id, clarified frontend-only access with exact frontend CORS, then approved public API denial and native execution. The owner explicitly resumed after a timed-out approval with “Lanjut task yang berhenti tadi”; the subsequent system-file install was explicitly approved and completed. No deployment branch merge/push was requested.

- Backend worktree /home/ubuntu/imt-smart-teens/api/.worktrees/production-deployment, branch feat/production-deployment, base3957fc16aa4ada5ed1bdb4159028dcd4374c0103. Trusted-proxy implementation commit4f39a82; packaging closure commits are recorded in Git.
- Frontend worktree /home/ubuntu/imt-smart-teens/web/.worktrees/production-deployment, branch feat/production-deployment, based985ff95ab925229ef1f19516c81b471aa12b38f.
- Native implementer/orchestrator Mochi, session model gpt-6.1-sol; exact effort unknown. No per-task implementation/review subagents or claimed Luna/high routing for this milestone. One whole-change reviewer follows.
- Binding spec/plan: docs/superpowers/specs/2026-11-16-production-compose-domains-design.md and docs/superpowers/plans/2026-11-16-production-compose-domains.md. Durable operator guide: docs/deployment.md.

## Live result and image provenance

- Frontend https://imtsmartteens.my.id: public compiled SPA, same-origin /api/v1 proxy.
- API https://api.imtsmartteens.my.id: valid certificate,403 application paths, ACME-only exception. Browser CORS origin is exactly https://imtsmartteens.my.id, not wildcard/API-host origin.
- Compose project imt-production: postgres/backend/frontend healthy; frontend publishes only127.0.0.1:5180. Backend4180 and PG5432 have no published host ports. Only frontend has an ingress-capable edge network; app/database networks remain internal.
- App subnet172.30.251.0/28, trusted frontend172.30.251.2, backend172.30.251.3. Private DB network is separate. Host edge overwrites IP headers; arbitrary client headers cannot evade existing limits.
- Observed backend runtime image sha256:8a93c7e81059961540c4dcf66108651a250cb2481f8227356fb9fd23577ad0d2, user10001:10001.
- Observed frontend runtime image sha256:fb90c439fbef33258af21bad22c570e0a1ae60724023e25d4c66310e43bcff11, user101:101. Source-only docs/browser-check additions do not alter the served build/proxy bytes; read back index/assets were compared against the live image.
- Official base image index digests: postgres17-bookworm40865bc877a289da75963d97d72b59633f639fc5af7a4e856bfa3714cae5b536; Rust1.100.0-bookwormade39a710ef85f3506cffcfec3e8a24ba090badef1dff2fb798f71d878833fea; Node26.7.0-bookworm-slim4db36457f406501e6f608802e5da617e5fbd0e80b75901b6a09de1ae5a667d32; unprivilegedNginxstable-alpinef5116ed4606f3eb015dd4a8e28cb9b351476dc0ad4bc6e9f93b19cec130cdf03; Debianbookworm-slim734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc. Exact sha256-prefixed references are pinned in Dockerfiles/Compose; no secret values are build arguments.

## Data and private operation

Production is a new volume imt-production_postgres_data, not the existing foundation/dev/E2E data. Read-only production check returned28 business tables,5 migrations,0admins,0students,0schools,0lessons,2emptybanks,1pathdraft. Normal anonymous CSRF context records can exist from real visits. No teaching fixtures or default accounts were copied/created. Enrollment requires privately provisioned real admin/school/content/grants first; deployment availability is not teaching-content readiness.

Settings: /home/ubuntu/.config/imt-smart-teens/production/compose.env (nonsecret paths/ports/project, mode600). Secret directory underneath,700/files400 with service UID ownership; secret/authentication/connection values [REDACTED]. Complete existing secret sets are preserved; invalid/partial sets are rejected, not rotated silently. Migrator and runtime roles are separate; runtime owns no application DDL permission or migration-ledger access. Root initialization is an explicit private SQL operation, not a production-mode bypass of the development-only CLI.

Separate smoke project imt-production-smoke uses port5182/subnet172.30.252.0/28 and its own volume/secrets. Its three containers were stopped after final verification to release resources; volume/backups/secrets were retained, not deleted. Tests refuse production targets. A synthetic school-only persistence probe was created there; admin/student fixture counts remained0. All existing foundation/dev data and portfolio files remain preserved. Deployment worktrees are live read-only bind-mount sources and must not be deleted during Git cleanup.

## Domain/TLS and rollback state

- Google public DNS A for both names and observed egress IPv4:43.133.132.59; no advertised AAAA. Both IPv6 localhost SNI virtual hosts separately tested with valid TLS, so lack of public IPv6 is not misrepresented as off-host IPv6 coverage.
- Added only /etc/nginx/sites-available/imtsmartteens and api-imtsmartteens plus matching symlinks. They were absent beforehand. Portfolio config was backed up under production/host-backup and remains byte-identical; public body SHA256dc620b9b02378429cb1e8dbb81d1d60a3467ddf0f37306d5af00c854d4dc96a4 unchanged.
- Dedicated ACME webroot /var/www/imt-smart-teens-acme; HTTP preflight marker read200 through both hostnames. Existing Certbot account reused; no new contact/production registration invented.
- Real certificate covers both exact SANs, expires2027-02-14. Final vhosts installed only after nginx-t, then reloaded. API root/api/admin/health403; frontend HTTP301 toHTTPS.
- Scoped executable renewal hook /etc/letsencrypt/renewal-hooks/deploy/50-imt-nginx-reload was installed after explicit resumed approval. Source/installed bytes matched; certbot.timer active/enabled; `certbot renew --cert-name imtsmartteens.my.id --dry-run --run-deploy-hooks --no-random-sleep-on-renew` exited0/all simulated renewals succeeded. nginx-t output on stderr was labeled hook error output by Certbot despite successful exit, not a failed hook.
- UFW currently active allowing SSH and NginxFull v4/v6; not changed here. No cloud-provider firewall audit is claimed.
- Rollback: restore prior IMT files if present; because these targets were new, disable only those two symlinks, validate/reload Nginx, and stop only the new stack if needed. Keep volume, backups, secrets, certificates and source bind mounts. Never change/remove portfolio/default/shared TLS or run production down -v.

## Evidence and verification

- Baseline65Rust and40Vitest+6legacy passed before changes.
- Proxy policy RED3failed/1existingpassed; focused GREEN4passed; full69Rust/0failed/0ignored, fmt/all-target warnings-denied Clippy/lockedbuild. Raw GREEN log scratch/imt-deploy-task1-green.log; initial RED result is recorded in native ledger/session tool output, not a fabricated raw log.
- Packaging first RED5infra/3FE missing-artifact assertions; all implemented. Additional Vite allowed-host/TLS, quoted tmpfs and frontend ingress-network regressions were each observed failing before correction.
- Before the final correction, infra11passed; after correction, the complete infra suite16passed. Frontend44Vitest+6legacy/build passed. Existing two Router warnings disclosed. Actual backend/frontend Docker builds succeeded, not only Dockerfile validation.
- Real smoke migrations and empty-root init repeated without resetting data. Runtime SET ROLE then CREATE/ALTER/migration-ledgerUPDATE received actual42501. Real cookies/CORS/CSRF and distinct IPv4/IPv6 login buckets verified through actual frontend socket. Committed synthetic data and secret hashes survived full container recreation; no hashes or secret values printed.
- Smoke custom-format backup created privately, catalog inspected, restored into a unique owned probe database, and verified synthetic data/five migrations/admin0/student0. Only that probe DB was dropped afterward. This is real restore evidence, not just a file-exists check.
- `tests/deployment/public.py --exercise-rate-limits` passed real CA/hostname/TLS1.3/SANs, redirects, exact container index/assets, SPA/API, API403, IPv6 localhost SNI routing, secure host-only cookie attributes, allowed/foreign Origin, structured Rust413 and inability to evade public login limits using forged IP headers. Ordinary public checks repeated after renewal also passed. These requests originate on the VPS, not an independent off-host probe.
- Frontend `deploy/verify-public.mjs` passed real Chromium at1440desktop/390mobile, correct /admin/masuk navigation/empty validation, loaded images and no horizontal overflow; JavaScript page exceptions0 and productionPOST/PUT/DELETE0. Four expected anonymous /me401 console entries are recorded, not hidden. Four actual screenshots/report under ignored test-results/deployment-public; parent visually inspected desktop student/mobile admin. Desktop long-title within-word wrap is a baseline visual qualification, not a new functional deploy failure. No credentials were typed or captured.
- Actual authenticated learning/editor/native200% zoom evidence belongs to the previous synthetic full-stack milestone; no claim that those authenticated flows were exercised against this intentionally empty production dataset.

## Final review and bounded correction

Independent reviewer sa-0-91b074b1/deleg_561f5351 reviewed both complete deployment ranges:0Critical/1Important/0newMinor, verdict With fixes. The parent accepted I1: a misconfigured root-run secret directory could chmod/chown its unrelated parent before rejecting it. No damage to the current production path was found or alleged. The original independent report is preserved under docs/reviews/2026-11-16-production-deployment.md; it is not relabeled as a second approval after correction.

ONE native correction pass restricted secrets to the OS operator's dedicated home namespace, production/smoke stages only; rejects traversal/symlink ancestry/wrong ownership/unsafe permissions/partial sets/invalid metadata before any mutation; never repairs existing shared/private parents. Existing operator-primary-group access on intermediate namespace directories remains unchanged; actual private stage/secret directories stay700, secretfiles400. Malicious concurrent trusted host operators are not an isolation guarantee. Five regression/control tests use non-root owned temporary trees and intercepted mutators: unsafe system-path strings never reach real system writes. Initial4negative tests produced7subcase failures; a sandbox UID-control error in the expanded RED log is distinguished from those meaningful failures. Final5tests passed; full infrastructure16passed. Logs scratch/imt-secret-preparation-red.log and imt-secret-preparation-green.log preserve actual output.

Parent also ran the fixed helper against the already valid production and smoke sets, comparing secret bytes and metadata/parent UID/GID/mode/mtime/ctime in memory: both unchanged, no values/hashes printed. No container rebuild or service restart was needed: this is an operator helper, not embedded Rust/served frontend. Fresh complete correction gates:69Rust/0failed/0ignored, fmt/Clippy/lockedbuild,16infra; frontend44Vitest+6legacy/build with the same two disclosed warnings. Full gate log scratch/imt-deploy-correction-gates.log. Parent closes I1 based on the reproduced regression, guard inspection and successful whole suites; no second review loop is claimed.

## Rulings I made

1. Trust only exact configured socket peers and one valid X-Real-IP, default disabled — preserves real visitor budgets without trusting public header chains — cost if wrong: adjust the deployment proxy contract, never bypass CSRF/rate limits.
2. Preserve the development-only bootstrap guard and use explicit private SQL for production empty roots — avoids an environment-mode bypass/public seeding endpoint — cost if wrong: a future dedicated operator CLI may be needed.
3. Quote the complete tmpfs string — real Compose flow-list parsing split comma options into invalid mount paths — cost if wrong: startup fails safely; exact normalized configuration regression and real container creation cover it.
4. Add an ingress-capable network only to frontend — real Docker29 suppressed publication for internal-only frontend despite HostConfig bindings — cost if wrong: frontend has limited egress, not backend/database/public API access.
5. Keep proxy body budget1MiB above Rust64KiB — preserves structured Rust413 instead of an early proxy HTML error — cost if wrong: larger bodies reach the still-bounded Rust policy; actual65537byte test passed.
6. Scope certificate renewal reload to the IMT lineage and validate first — webroot/certonly has no installer reload behavior — cost if wrong: stale served certificates; real dry-run/deploy-hook proof now passed.

7. Restrict privileged secret preparation to the operator's dedicated home production/smoke namespace, with all validation before writes and no existing-parent repairs — closes I1 without global permission changes — cost if wrong: relocation/permissions require deliberate operator review; no arbitrary-path bypass or silent rotation.
8. Keep real admin/school/content/grant setup as a separate owner operation — empty production data is intentional, not an invitation to copy fixtures — cost if wrong: enrollment waits for correct provisioning.
9. Do not promote prior authenticated synthetic learning/editor evidence into authenticated production proof — production fixtures/authentication artifacts were prohibited — cost if wrong: authorized real-operator acceptance is still needed.
10. Keep privacy/retention/child-data, accessibility/pixel-perfect, capacity and multi-replica limiter certification outside this deployment — those audits/designs were not performed — cost if wrong: assess/remediate before making those broader claims.
11. Do not claim independent off-host/cloud-firewall/public-IPv6 coverage — probes originate on this VPS and no AAAA is advertised — cost if wrong: separately verify external routes/policies before changing DNS/network assumptions.
12. Leave the seven existing application Minors in their prior explicit backlog — they are not deployment regressions or silently fixed — cost if wrong: a separately scoped correction is required, not an unannounced bonus rewrite.
13. Do not auto-repair already lost secret sets or incompatible future schema releases — preserving data/secrets is the authorized boundary — cost if wrong: a separate recovery/release decision and tested migration are required.

## Final limitations and integration

No Critical/Important deployment finding remains after the parent's verified correction; no new Minor was raised. The previous seven Minors remain: CMS multiquery detail metadata consistency, unfinished dashboard fallback copy, raw wire-enum presentation, missing unset-password case, wrong-password smoke classification, two Router warnings, and report-only historical-version UUID UX. Their full effects/dispositions are in the earlier full-stack final handoff.

Native execution intentionally used one independent whole-change review and one RED/GREEN material correction, not an invented subsequent reviewer PASS. Source/application packaging remains on both feat/production-deployment branches, not main. Preserve runtime bind-mount worktrees and private operational state. Real admin/school/content/grants, broader governance/audits and independent external availability remain the stated owner gates. The next integration choice is local main merge, push/PR, or keeping branches as-is; no automatic integration is authorized.

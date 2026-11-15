# postgres-foundation — Local PostgreSQL and backend contract design

- Date (Asia/Jakarta): 2026-11-15
- Status: ready_for_review
- Context / objective: Implement the approved PostgreSQL-first plan, API inventory and database design, including admin terminology and school CMS.
- Owner / implementor: Mochi, inline gpt-6.1-sol; exact reasoning effort unknown.
- Reviewer: pending independent read-only review.
- Delegation evidence: no Luna implementation was dispatched. The owner explicitly requested superpowers:executing-plans (inline execution), superseding the earlier implementation routing for this task. Native per-child model/effort selection is unavailable; no model switch is claimed.

## Scope and contracts

- Approved plan: /home/ubuntu/.hermes/plans/2026-11-15_115310-imt-api-database-postgres.md.
- Canonical rules: fajarstyuu/imt-smart-teen at 8df5bafbe8912e37d2e0ba898732bc9f5d9159b7; AGENTS.md, docs/rules.md, docs/backend.md, docs/frontend.md, docs/delegation.md, docs/handoff.md, latest relevant docs/handoffs/2026-11-14-imt-web-routing-brand.md.
- Allowed paths: compose.yaml, .env.example, .gitignore, tests/infra/, docs/api-inventory.md, docs/database-design.md, docs/postgres-local.md, this handoff, readme.md.
- Backend repository root replaces the specification's conceptual backend/ directory; do not create backend/backend or empty Rust scaffolding.
- Runtime milestone: one local-development PostgreSQL service, required password, loopback binding, persistent volume and actual authentication/persistence verification.
- API/CMS/schema are design deliverables only. No Rust endpoints, business migrations, frontend edits or production rollout in this change.
- Owner clarified admin instead of teacher and school add/edit/delete CMS. School deletion archives records, preserving historical results; student registration target uses active school IDs.
- Host installation and worktree were explicitly approved through the clarification UI. Docker daemon commands use sudo; user not added to docker group.

## Files and entry points

- Branch: chore/postgres-foundation; baseline 40bb0f5b61cfd7b4c0a69d4dd141bc5818a63389.
- Worktree: /home/ubuntu/imt-smart-teens/api/.worktrees/postgres-foundation.
- Existing backend file inspected: readme.md (Hello World); no manifests or application suite exist.
- Frontend checked: src/services/api.ts and source contracts from web SHA 6344f0b7304d9cfcd5b642034ccf33b2c3393585. Legacy teacher paths must be documented as existing source, not silently renamed in evidence.

## Verification

- Initial Docker absence confirmed. Approved official apt installation completed; Docker 29.9.3, Compose v5.7.1, sudo docker info succeeds.
- No existing container runtime packages found during conflict check; port 55432 had no listener.
- Git worktree confirmed on chore/postgres-foundation; main remains untouched.
- Compose TDD: 3 tests first failed because compose.yaml was absent; same tests passed after implementation. Command: `python3 -m unittest discover -s tests/infra -p 'test_*.py' -v`.
- Runtime RED: `sudo env IMT_ALLOW_DB_SMOKE=1 bash tests/infra/smoke_postgres.sh` failed with service postgres not running before first startup.
- Runtime GREEN: actual PostgreSQL healthy; smoke passed TCP SELECT 1, wrong-password rejection, sentinel persistence through force-recreate and cleanup. Repeated successful smoke runs; no business schema migrated.
- `sudo docker compose --env-file .env port postgres 5432`: 127.0.0.1:55432.
- Linux host-published-port SQL via temporary postgres client container with --network host: SELECT 1 returned 1.
- `stat -c '%a %n' .env`: 600 .env; .env ignored, .env.example not ignored; no secret values printed or tracked.
- `git diff --check`: clean; API/DB decision checks passed, 28 proposed tables verified by extraction. These documentation checks do not establish endpoint/constraint behavior.
- Resolved PostgreSQL image: postgres@sha256:40865bc877a289da75963d97d72b59633f639fc5af7a4e856bfa3714cae5b536.
- Runtime project postgres-foundation; named volume postgres-foundation_postgres_data; private local development DB remains running.
- Host: nginx and Docker services active. Existing UFW rules untouched; binding is loopback, not a reliance on UFW to filter Docker traffic.
- Cargo/frontend tests: not applicable; neither application implementation nor frontend edits in scope.

## Review and next steps

- Review status: pending.
- Next: independent read-only whole-branch review, address material findings with RED/GREEN tests, run final verification and finalize this handoff. No push/merge authorized.
- Remaining production gates: least-privilege roles, backup/restore, retention/access administration, deployment hardening.

# full-stack — Rust backend and React integration

- Date (Asia/Jakarta): 2026-11-15
- Status: in_progress
- Objective: implement the approved v1 API/database design and connect the existing frontend, including admin role and school CMS.
- Owner/orchestrator: Mochi; current session gpt-6.1-sol, exact reasoning effort unknown.
- Implementation/review agents: assigned per scoped tasks; actual IDs and test evidence recorded as returned. No Luna/high routing is claimed.
- Authorization: owner requested local foundation merge followed by backend/frontend work; explicitly approved implementation subagents using available models in clarification UI.

## Scope and source

- Foundation fast-forward merged into local backend main at d3327ae49ed7dd153662ec29d432fb78470c9f43, merged-tree tests 3/3 passed. No push performed.
- Spec: docs/api-inventory.md and docs/database-design.md, plus canonical AGENTS.md, rules/backend/frontend/delegation/handoff documents in /home/ubuntu/imt-smart-teens/rules at 8df5baf.
- Plan: docs/superpowers/plans/2026-11-15-full-stack.md. Separate sequential backend foundation, student workflow, admin CMS/reporting, frontend integration and final verification/review tasks.
- Backend feature branch feat/full-backend in /home/ubuntu/imt-smart-teens/api/.worktrees/full-backend.
- Frontend feature branch feat/rust-api-integration in /home/ubuntu/imt-smart-teens/web/.worktrees/rust-api-integration; source base 6344f0b.
- This new milestone authorizes business migrations/backend/frontend implementation; previous foundation's scope limitation remains historically accurate.
- API loopback 4180, frontend loopback 5173; no public rollout, nginx edits, new feature-branch merges or remote pushes authorized.

## Prerequisites and safety

- Rust1.100.0 installed under ~/.cargo; use single-job builds and no debug info due limited RAM.
- Existing local PostgreSQL17 service retained. Parent created separate migrator/app/test roles and isolated test database. Runtime and test environment files are ignored and mode0600; no secret values recorded here.
- Do not touch existing application data or destroy volumes. Early tasks run migrations/tests only against owned test DBs; application migrations occur after review through parent.
- Core architecture remains modular monolith with inward DDD dependencies, SQLx adapters, CSRF/session auth, immutable versioned content and transactional concurrency protection.

## Verification and review

- Foundation merge verified with all 3 existing tests passing and branch diff whitespace clean.
- New Rust/API/frontend behavior: not yet implemented or verified at this handoff creation.
- Task/final review status: pending. Feature must not be marked complete until real Rust/PostgreSQL and browser integration are exercised.

## Next action

- Assign Task1 backend foundation to an isolated implementation subagent with exact path ownership and TDD/report requirements.

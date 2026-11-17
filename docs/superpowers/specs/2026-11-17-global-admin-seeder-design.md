# Global admin and private seeder design

Date: 2026-11-17 (runtime UTC+08:00).
Status: owner approved the global-admin model and this implementation/production rollout plan with Gas. Implementation evidence and final acceptance live in the dated handoff.

## Owner-approved intent

There is one administrator role: every active admin manages all schools, learning content and question banks and reads all student reports across existing and future schools. There is no per-school administrator or separate superadmin tier. The owner explicitly corrected the earlier mixed catalog/content/global and reporting/grant model; this decision supersedes that earlier scope policy, not student privacy protections.

The requested production email is provided privately by the operator, never hardcoded in the seeder, fixtures or documentation. Display name defaults to Admin unless the owner supplies another. The owner wants a strong but typeable generated password delivered after account creation and login verification.

## Scope and behavior

- Require a current active admin session at every administrative HTTP boundary. Student/anonymous/inactive/revoked/expired sessions remain denied; CSRF, rate budgets, audit events and private field allowlists stay enforced.
- Remove catalog/content booleans and per-school grants as effective authorization gates. Global access must be implemented in backend services AND all report projections, including counts, pagination, details, attempt answers and historical question-version answers. School/date/stage/version filters still constrain results.
- Keep legacy DB columns/grant rows intact but unused for authorization in this compatibility release; do not rewrite existing migrations, drop data, change existing password hashes or manufacture grants for every current school. No schema migration is needed. Legacy admin DTO permission booleans reflect global capabilities; school_ids is deprecated and must not drive authorization or a complete school picker.
- Update current frontend navigation/guards and report wording. Report filtering uses bounded paginated school catalog reads, real school names, active/archived schools and preserved selected URL IDs. New schools become visible without re-login or account-grant edits. No bulk all-school bootstrap, invented new account-management CMS or unrelated UI redesign.
- Add an explicit private seed-admin CLI, preserving existing provision-admin create-only behavior. Seeder requires operator-provided normalized email, name and password via private stdin/file. First creation stores an Argon2id hash; repeat execution keeps ID/hash/name/active state unchanged. Concurrent same-email execution creates exactly one row; conflicting or inactive existing accounts are reported unchanged, never reset/reactivated silently. Existing and created outcomes are distinct; never claim a newly generated password works for a preserved account.
- Seeder is never invoked by ordinary serve, migrations, image build or HTTP. No public registration endpoint for admins. Do not embed the requested email or plaintext password in source, Docker configuration/arguments, Git, screenshots or logs.
- For the owner's new account, generate a CSPRNG passphrase from at least six independently chosen familiar ASCII Indonesian words from a validated unique list of at least128 words plus a random numeric suffix. Keep it in memory/private operator storage only; show it to the owner only after the created account successfully authenticates through public HTTPS. Do not promise an in-app password-change feature that does not exist.

## Rollout and acceptance

Work on isolated backend/frontend branches from API8e01eda and FEe3574bc. Test cross-school/new-school/archived-school access and denied non-admin sessions in owned synthetic databases first. Test seeder rerun/concurrency/privacy and actual public login after production provisioning. Production must retain its volume, secrets, five migrations, teaching data, domain policy and portfolio. Back up privately before the authorized production write. Build reviewed images serially; capture prior image IDs for rollback. Preserve existing live worktrees and bind mounts; do not silently repoint private settings to temporary feature checkouts. No merge/push authorization is inferred from this feature request.

The earlier seven Minor backlog items are not blanket permission for unrelated polish. One native implementation plus one final independent whole-change review is recommended, consistent with the owner's previous execution choice; any material correction is bounded and backed by RED/GREEN tests.

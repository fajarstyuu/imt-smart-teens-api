# Production deployment: IMT Smart Teens

## Topology and boundaries

The owner-approved deployment has three long-lived Compose services: frontend, backend and PostgreSQL. Host Nginx owns TLS and HTTP redirects. The frontend container serves the compiled Vite SPA through non-root Nginx and proxies `/api/v1/` to the private backend; Vite development/preview is not the production server.

- Frontend: `https://imtsmartteens.my.id`, same-origin browser API calls.
- API hostname: `https://api.imtsmartteens.my.id`, valid TLS but application paths deliberately return403. Only ACME challenge files are exempt. This is not an alternate public backend ingress.
- Production frontend binds only `127.0.0.1:5180`. Backend4180/PostgreSQL5432 have no published host ports. Internal app/database networks isolate backend and database; frontend alone also joins an ingress-capable edge network so Docker can publish its loopback port.
- CORS is exactly `https://imtsmartteens.my.id`; production Secure/HttpOnly/SameSite=Lax host-only cookies and CSRF remain enforced. CORS is a browser response-access policy, not a firewall. Public frontend API routes still require the existing authentication/authorization where applicable.
- Host Nginx overwrites client-IP headers; frontend passes the sanitized value. The backend trusts only its configured frontend TCP peer and one valid X-Real-IP. The default allowlist is empty; arbitrary Forwarded/X-Forwarded-For values cannot select rate identity. Limits remain10admin-login/300API perIP per60seconds.

## Prerequisites and source layout

Use reviewed API/frontend checkouts, existing Docker Engine+Compose, host Nginx and an authorized Certbot account. DNS A/AAAA must point to the actual server before issuance. Do not overwrite another site's virtual host or certificate configuration.

The backend `deploy/compose.production.yaml` contains the stack; set `FRONTEND_BUILD_CONTEXT` explicitly to the frontend checkout. Relative sibling paths are not guessed. The current deployment uses isolated `feat/production-deployment` worktrees in both repositories. Keep them while PostgreSQL init scripts/SQL are read-only bind mounts from the backend checkout.

Official builder/runtime image manifests are pinned by digest in Dockerfiles/Compose. Rust1.100.0, lockfiles, serial builds and bounded runtime resources suit this approximately2GiB host. Backend runs10001:10001; frontend101:101; PostgreSQL bootstrap uses the verified image's postgres UID. No privileged application container or Docker socket mount exists.

## Private settings and secrets

Copy `deploy/.env.production.example` to a private operator settings file, e.g. `/home/ubuntu/.config/imt-smart-teens/production/compose.env`, mode600. It contains paths, project name, image tag, subnet and port only, not passwords. Set `COMPOSE_PROJECT_NAME=imt-production`, `FRONTEND_BUILD_CONTEXT`, and `IMT_SECRET_DIR` deliberately. Check selected subnets against live routes and Docker networks. Defaults are app172.30.251.0/28/frontend172.30.251.2/backend172.30.251.3.

Secret preparation accepts only `<operator-home>/.config/imt-smart-teens/{production,smoke}/secrets`, using the OS account home and matching primary group, not an arbitrary root-supplied parent. It rejects relative/parent-traversal paths, symlink ancestry, unknown/partial sets, unsafe ownership/permissions and metadata before any mutation. Existing `.config`/namespace directories retain their ownership/modes; access by the trusted operator primary group is permitted there, but world-writable/foreign-writable ancestry is rejected and the actual production/smoke private parent must be700. This is not isolation from a malicious concurrent trusted host operator. Arbitrary relocation requires an explicitly reviewed deployment move, not a bypass flag.

Run setup as the intended operator account; the script escalates only the actions needing root. Before first copying settings, create the dedicated private parent as that non-root account:

```sh
install -d -m700 "$HOME/.config/imt-smart-teens/production"
```

Do not chmod/chown shared system parents to repair a configuration error. An existing private-parent permission mismatch requires explicit operator review. `operations.sh prepare` invokes the explicit root/operator secret generator. A complete new set is published atomically; a complete existing set is validated/preserved without changing any existing directory or file; a partial/unknown set is rejected, never silently regenerated. Parent/secret directories are700. Secret files are400 and owned by the numeric service UID that reads them; metadata is600. This is required because local Compose file-backed secrets are bind mounts, not a reason to make passwords world-readable. Values never appear in CLI arguments, committed files, screenshots or logs.

PostgreSQL gets only its owner/app/migrator password files; backend gets only its DML database URL and CSRF key; the operations-only migrator gets only its own connection secret. Private entrypoint file loading preserves signals with exec and rejects ambiguous environment/file sources. Connection/authentication values are [REDACTED]. Do not run Docker inspect with full environment output or print secret files.

## Build and first start

Run from the backend deployment checkout, with the frontend context matching the reviewed frontend checkout:

```sh
export IMT_ENV_FILE=/private/path/to/production/compose.env
sudo docker compose --env-file "$IMT_ENV_FILE" -f deploy/compose.production.yaml build backend
sudo docker compose --env-file "$IMT_ENV_FILE" -f deploy/compose.production.yaml build frontend
sh deploy/operations.sh prepare
sh deploy/operations.sh migrate
sh deploy/operations.sh initialize-empty
sh deploy/operations.sh up
sh deploy/operations.sh status
```

The image tag in private settings selects the locally built backend/frontend release. Do not change it without building/verifying the corresponding images. Never run competing Rust image builds on the small host.

Initialization is explicit, never part of ordinary `serve`:

1. New PostgreSQL volume bootstrap creates limited migrator/app roles, revokes public schema CREATE and sets default runtime DML grants on migrator-owned application tables.
2. Existing Rust migration CLI runs under the migrator role; runtime does not inherit that role. The operator revokes runtime access to `_sqlx_migrations` after migration.
3. An explicit private SQL operation serializes empty-content initialization, preserves existing roots/drafts, rejects ambiguous configuration, and creates only missing paired/quiz roots and one path draft. It does not bypass the development-only CLI environment guard.
4. Ordinary initialization never invents admin accounts or teaching data. Use the explicit private seed-admin operator command for an authorized account; every active admin has global school/content/report access without school grants. Real schools and published teaching content remain separate owner operations.

Never copy synthetic integration fixtures into production. Never run `down -v` on production or any existing development project. Existing foundation/application/E2E data stays separate.

## Host domains and certificates

Reviewed host templates are in `deploy/host-nginx/`. Install only the two new IMT site targets and matching symlinks; preserve the portfolio/default site. Save exact prior targets or record their absence in a private backup before changing them.

1. Stage `bootstrap-http.conf.template` while no certificate exists. It serves only the dedicated ACME webroot and returns503 for application paths; do not expose the application over temporary plain HTTP.
2. Run `nginx -t`, reload only on success, and read a unique challenge marker through both public hostnames.
3. Issue one SAN certificate for both exact names using the existing authorized account and `/var/www/imt-smart-teens-acme`. Do not invent contact details/accept a new account's terms without authorization.
4. Install frontend/API TLS templates, run `nginx -t`, then reload. TLS includes the existing managed TLS1.2/1.3 options; frontend proxy preserves Origin and overwrites incoming IP headers. The API template has no backend upstream.
5. Install the executable `renewal-hook.sh` as `/etc/letsencrypt/renewal-hooks/deploy/50-imt-nginx-reload`. It reloads only the IMT certificate lineage and only after nginx validation. A webroot/certonly renewal does not itself install an Nginx reload handler.
6. Verify the existing Certbot timer and execute:

```sh
sudo certbot renew --cert-name imtsmartteens.my.id --dry-run --run-deploy-hooks --no-random-sleep-on-renew
```

Actual acceptance: both SANs issued successfully, expiry2027-02-14; renewal simulation and scoped deploy hook succeeded, timer active/enabled. Certbot labels normal nginx-t stderr as hook error output; the command still exited0 and reported all simulated renewals successful. Do not mistake that label for a failed renewal, or claim success from a timer alone.

## Verification

- `cargo fmt`, warnings-denied all-target/all-feature Clippy, real PostgreSQL cargo suite and locked build:69Rust tests passed after the new exact-peer proxy policy.
- `python3 -m unittest discover -s tests/infra -p 'test_*.py' -v`:16passed after the one material correction, covering Compose, secret mounts, role/init boundaries, exact tmpfs parsing, ingress/private networks, host policy/renewal and privileged-preparation rejection/no-mutation/idempotence. These static checks complement, not replace, container execution.
- Frontend `npm test`/`npm run build`:44Vitest +6 explicitly legacy-domain tests passed; two pre-existing Router bundling warnings remain. Legacy tests are not real persistence proof.
- `tests/deployment/smoke.py --env-file /private/smoke/compose.env`: real isolated containers, migrations/init repeated, actual42501 CREATE/ALTER/migration-ledger denials, cookie/CORS/CSRF, independent IPv4/IPv6 rate buckets, and data/secret persistence across container recreation. It refuses production project/secrets. Use only its separately named smoke database/volume and port5182.
- `tests/deployment/backup.py --backup-file /private/smoke/backups/file.dump`: private dump catalog and actual restore into a new owned smoke database, with synthetic data/five migrations retained; only its unique probe database is dropped afterward. Do not point it at production backups.
- `tests/deployment/public.py`: real CA/hostname validation, SANs, redirects, exact live container index/assets, SPA/API, API403, secure cookie scope, CORS/CSRF, body budget and portfolio preservation. The optional `--exercise-rate-limits` intentionally consumes invalid-login budget, so do not run it casually during real operator login. Tests are host-originated public probes, not an independent off-host availability audit. A legitimate portfolio update changes the historical comparison hash; capture/review a new baseline before using that assertion on a later deployment.
- Frontend `deploy/verify-public.mjs`: actual anonymous Chromium desktop/mobile page, correct admin navigation, empty-form validation, loaded imagery and no horizontal overflow; screenshots/report in ignored `test-results/deployment-public`. It creates no accounts/content and performs no production POST/PUT/DELETE. Anonymous `/me`401 console messages are expected; JavaScript page exceptions were zero. Full authenticated learning/browser/zoom behavior was verified in the previous isolated integration milestone, not re-created with production fixtures.

## Backup, recovery and rollback

```sh
IMT_ENV_FILE=/private/production/compose.env sh deploy/operations.sh backup
```

Backups are private custom-format dumps under the settings/secret parent. A failed pg_dump exits unsuccessfully and may leave an incomplete file: validate the catalog and restore before treating a backup as usable. Restore testing must use an explicitly new owned database, never overwrite production in place. Restore roles/ownership/default grants as part of a separately authorized recovery; preserve the operator secrets needed to reconnect.

For a failed IMT routing cutover, restore prior IMT vhost targets if they existed; otherwise disable only the two newly created IMT symlinks. Run nginx-t before reload. Do not edit the portfolio/default site or shared TLS files. Stop only the new Compose project if necessary, leaving volume and secret files intact. Do not delete live worktrees/bind mounts during Git cleanup. Certificates and private operational state are not disposable build artifacts.

This deployment does not establish privacy/retention/child-data governance, capacity certification, shared rate limiting across replicas, a standalone accessibility certification, or the elimination of the previous seven Minor items. Those remain explicit owner/backlog decisions.

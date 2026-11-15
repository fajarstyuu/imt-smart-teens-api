# IMT Smart Teens API

PostgreSQL-first development foundation and backend contract design for IMT Smart Teens.

**Implemented:** private PostgreSQL 17 Compose service, configuration checks, and real TCP authentication/persistence smoke test.

**Not implemented yet:** Rust/Axum API, application database tables/migrations, live CMS endpoints, or frontend integration. The API inventory and proposed 28-table database model are designs, not running application features.

## Documents

- [Local PostgreSQL setup, operations and verification](docs/postgres-local.md)
- [API inventory and frontend migration map](docs/api-inventory.md)
- [Database design and transaction boundaries](docs/database-design.md)
- [Feature handoff and actual verification evidence](docs/handoffs/2026-11-15-postgres-foundation.md)

CMS design covers **admin**, schools (add/edit/archive), lesson content/order/publication, one shared pre/post-test question bank, a separate quiz bank, and scoped student-result reporting. School archival never deletes historical student records.

## Quick start

Requires Docker Engine, Compose plugin, Bash, and Python 3.10+. Run from the directory containing `compose.yaml`. Follow the [runbook](docs/postgres-local.md#first-time-environment) to create a private `.env` with a generated password; do not leave the example password empty or overwrite an existing local file.

On the verified VPS Docker daemon commands require sudo; no root-equivalent docker-group membership was added.

```bash
sudo docker compose --env-file .env config --quiet
sudo docker compose --env-file .env up -d --wait --wait-timeout 120 postgres
sudo docker compose --env-file .env ps
sudo docker compose --env-file .env port postgres 5432
```

Expected healthy database bound only to `127.0.0.1:55432` by default. Password is local to the ignored `.env`; never post it or expanded Compose/container configuration in logs.

## Tests

```bash
# Does not require daemon access; uses synthetic fixture credentials.
python3 -m unittest discover -s tests/infra -p 'test_*.py' -v
bash -n tests/infra/smoke_postgres.sh

# Opt-in, development only: writes a synthetic schema and recreates the container.
sudo env IMT_ALLOW_DB_SMOKE=1 bash tests/infra/smoke_postgres.sh
```

The runtime smoke checks successful TCP SQL, rejection of a wrong password and survival of a marker through container recreation. Do not run it on production/shared databases. See the runbook for interruption and data-safety details.

## Safety and next milestone

The Compose bootstrap account is a development superuser, not the future API login. Before implementing the API, provision separate migration/application database roles and build versioned SQLx migrations with real PostgreSQL tests. No `cargo test` or application build is advertised until a Cargo project exists.

Never delete the volume to fix credentials. `down -v` is destructive and requires separate approval. The named volume is not a backup; production needs reviewed secrets, network/TLS policy, retention and tested backup/restore.

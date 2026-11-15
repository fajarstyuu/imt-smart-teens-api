# Local PostgreSQL development foundation

This service is a development database, not a production deployment. No application tables, SQLx migrations or Rust API exist yet. Database/schema and API contracts are separate design documents.

## Prerequisites and topology

- Docker Engine and Compose plugin supporting `config --format json`, `up --wait`, and `--wait-timeout`.
- Python 3.10+ and Bash for tests. No pip packages or host PostgreSQL client required.
- Verified host installation: Ubuntu 24.04.4 LTS, Docker 29.9.3, Compose v5.7.1. Installed from Docker's official Ubuntu apt repository with owner permission.
- On this VPS, Docker daemon access requires `sudo`; the user was deliberately not added to the root-equivalent docker group. CLI-only config tests need no sudo.
- Run commands from the repository/worktree root containing `compose.yaml`. On this host: `/home/ubuntu/imt-smart-teens/api/.worktrees/postgres-foundation`.
- Host clients: `127.0.0.1:55432`, database `imt_smart_teens_dev`, bootstrap user `imt_dev` by default.
- A future API container on this Compose network must use `postgres:5432`; its own `127.0.0.1` is not the database.
- Compose project name derives from the working directory unless explicitly overridden. This worktree uses `postgres-foundation`; its named data volume is `postgres-foundation_postgres_data`. Keep project name and volume consistent when moving the checkout. Do not accidentally initialize a new volume and mistake it for data loss.
- PostgreSQL 17 uses `/var/lib/postgresql/data`; do not change the volume mount to a parent path or switch to 18+ without validating the image's layout and upgrade policy.

## First-time environment

Never overwrite an existing `.env`, print it into logs, or commit it. Create a new local file with a random hex password and mode 0600:

```bash
python3 -c 'from pathlib import Path; import secrets,os; p=Path(".env"); fd=os.open(p,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600); os.write(fd,("POSTGRES_DB=imt_smart_teens_dev\nPOSTGRES_USER=imt_dev\nPOSTGRES_PASSWORD="+secrets.token_hex(32)+"\nPOSTGRES_PORT=55432\n").encode()); os.close(fd)'
```

If it already exists, preserve values and verify permissions without displaying contents: `stat -c '%a %n' .env` (expected 600). Change `POSTGRES_PORT` only to another unused local port if needed. `POSTGRES_PASSWORD` is mandatory; an empty value causes Compose to reject startup.

`POSTGRES_USER` is the image bootstrap **superuser**, acceptable only for this local infrastructure milestone. Before integrating a real API, provision a distinct DDL migrator and DML-only application role; do not connect the API as this user.

Host-side application connection template (never commit a real value):

```text
postgresql://<user>:<URL-encoded-password>@127.0.0.1:<port>/<database>
```

Changing `.env` does not change credentials or the default database/user in an already initialized volume. Rotate credentials using an authorized SQL procedure with coordinated client changes; do not delete the volume to solve a login failure.

## Start and inspect safely

```bash
sudo docker compose --env-file .env config --quiet
sudo docker compose --env-file .env up -d --wait --wait-timeout 120 postgres
sudo docker compose --env-file .env ps
sudo docker compose --env-file .env port postgres 5432
```

Expected: healthy `postgres` service and `127.0.0.1:55432` by default. Do not post unredacted `docker compose config` or full container inspect output: both can expose environment secrets. Readiness alone is not a password-authentication or application-schema test.

## Verification

```bash
python3 -m unittest discover -s tests/infra -p 'test_*.py' -v
bash -n tests/infra/smoke_postgres.sh
sudo env IMT_ALLOW_DB_SMOKE=1 bash tests/infra/smoke_postgres.sh
```

The contract tests run the real Compose parser without contacting the daemon; fixture credentials are synthetic. They verify required password, loopback binding including port override, named persistent mount and readiness configuration.

The smoke test is **opt-in and development-only**: it performs a real TCP `SELECT 1`, rejects an incorrect password, creates a uniquely named synthetic schema/marker, force-recreates this project's PostgreSQL container, checks the marker survived, and cleans up its own schema. This interrupts connections to the development container. Do not run on a shared/production database. It does not delete volumes or business records. Expected final output: `PASS: TCP SQL, rejected wrong password, and persistent volume recreation`.

Optional host-published-port verification without installing psql on the host:

```bash
sudo docker run --rm --network host --env-file .env postgres:17-bookworm sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h 127.0.0.1 -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -Atc "SELECT 1"'
```

Expected `1`. This Linux-only client container uses host networking; no password is printed or put directly in command arguments. Other OS networking may differ.

## Stop, resume, and data safety

```bash
sudo docker compose --env-file .env stop postgres
sudo docker compose --env-file .env up -d --wait --wait-timeout 120 postgres
```

`docker compose down` removes this project's containers/network but retains the named data volume. **Never run `down -v` without separate explicit approval:** it destroys the stored database. Named volumes are not backups. This service remains running after verification; `unless-stopped` restarts it after a daemon/host restart unless deliberately stopped.

The port is bound only to host loopback, not a public address. UFW rules were not changed; do not assume UFW alone filters Docker-published ports. Review both binding and Docker networking before deployment.

## Image provenance and production gaps

Inspect a resolved image digest without exposing secrets:

```bash
sudo docker image inspect postgres:17-bookworm --format '{{json .RepoDigests}}'
```

The major/OS tag can receive updates; the actual digest used during verification is recorded in the feature handoff. Before production, adopt reviewed digest updates, least-privilege roles, secret injection, network/TLS policy, tested backup/restore, retention, resource sizing, monitoring and a migration/rollback strategy. This development database is not proof any of those gates are satisfied.

#!/usr/bin/env bash
# Opt-in dev-only test: writes one synthetic schema and recreates the DB container.
set -euo pipefail
cd "$(dirname "$0")/../.."
[[ "${IMT_ALLOW_DB_SMOKE:-}" == "1" ]] || { printf 'Set IMT_ALLOW_DB_SMOKE=1 for the dev-only persistence test.\n' >&2; exit 2; }
dc=(docker compose --env-file .env -f compose.yaml)
# exec requires an existing container: the initial RED run must fail here.
[[ "$("${dc[@]}" exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h 127.0.0.1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -Atc "SELECT 1"')" == "1" ]]
# Prove password auth over TCP; Unix-socket readiness is not enough.
if "${dc[@]}" exec -T postgres sh -c 'PGPASSWORD=deliberately-incorrect psql -h 127.0.0.1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Atc "SELECT 1"' >/dev/null 2>&1; then
  printf 'FAIL: wrong password accepted\n' >&2; exit 1
fi
schema="infra_smoke_$(python3 -c 'import uuid; print(uuid.uuid4().hex)')"
# Generated identifier contains only a fixed prefix and hex; never user SQL.
cleanup() {
  "${dc[@]}" exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h 127.0.0.1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' <<<"DROP SCHEMA IF EXISTS $schema CASCADE;" >/dev/null || true
}
trap cleanup EXIT
"${dc[@]}" exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h 127.0.0.1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1' <<SQL
CREATE SCHEMA $schema;
CREATE TABLE $schema.sentinel (id integer PRIMARY KEY, value text NOT NULL);
INSERT INTO $schema.sentinel VALUES (1, 'persisted');
SQL
"${dc[@]}" up -d --force-recreate --wait --wait-timeout 120 postgres
value=$("${dc[@]}" exec -T postgres sh -c 'PGPASSWORD="$POSTGRES_PASSWORD" psql -h 127.0.0.1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -At' <<<"SELECT value FROM $schema.sentinel WHERE id=1;")
[[ "$value" == "persisted" ]]
printf 'PASS: TCP SQL, rejected wrong password, and persistent volume recreation\n'

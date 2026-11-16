#!/bin/sh
set -eu
# Runs only on a fresh PostgreSQL volume; never resets existing role passwords.
APP_PASSWORD=$(cat /run/secrets/app_password)
MIGRATOR_PASSWORD=$(cat /run/secrets/migrator_password)
export APP_PASSWORD MIGRATOR_PASSWORD
# Keep connection/statement diagnostics containing secrets out of container logs.
if ! psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" >/dev/null 2>&1 <<'SQL'
\getenv app_password APP_PASSWORD
\getenv migrator_password MIGRATOR_PASSWORD
CREATE ROLE imt_migrator LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT PASSWORD :'migrator_password';
CREATE ROLE imt_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT PASSWORD :'app_password';
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT USAGE, CREATE ON SCHEMA public TO imt_migrator;
GRANT USAGE ON SCHEMA public TO imt_app;
ALTER DEFAULT PRIVILEGES FOR ROLE imt_migrator IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO imt_app;
ALTER DEFAULT PRIVILEGES FOR ROLE imt_migrator IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO imt_app;
SQL
then
    printf '%s\n' 'Private role bootstrap failed; refusing incomplete initialization' >&2
    exit 1
fi
unset APP_PASSWORD MIGRATOR_PASSWORD

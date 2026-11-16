#!/bin/sh
set -eu
# File values never enter Docker configuration, command arguments or logs.
load_secret() {
    name=$1 file=$2 current=$3
    [ -n "$file" ] || return 0
    [ -z "$current" ] || { printf '%s\n' "Ambiguous secret configuration: $name" >&2; exit 1; }
    [ -r "$file" ] || { printf '%s\n' "Required secret file unavailable: $name" >&2; exit 1; }
    value=$(cat "$file")
    [ -n "$value" ] || { printf '%s\n' "Required secret file empty: $name" >&2; exit 1; }
    export "$name=$value"
}
load_secret DATABASE_URL "${DATABASE_URL_FILE:-}" "${DATABASE_URL:-}"
load_secret MIGRATION_DATABASE_URL "${MIGRATION_DATABASE_URL_FILE:-}" "${MIGRATION_DATABASE_URL:-}"
load_secret CSRF_SIGNING_KEY "${CSRF_SIGNING_KEY_FILE:-}" "${CSRF_SIGNING_KEY:-}"
load_secret ADMIN_PASSWORD "${ADMIN_PASSWORD_FILE:-}" "${ADMIN_PASSWORD:-}"
exec /usr/local/bin/imt-smart-teens "$@"

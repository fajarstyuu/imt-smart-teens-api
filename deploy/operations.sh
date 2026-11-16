#!/bin/sh
# Explicit operator actions only. No destructive down/reset/volume-delete action exists.
set -eu
umask 077
DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
: "${IMT_ENV_FILE:?Set IMT_ENV_FILE to the private nonsecret Compose settings file}"
docker_cmd() {
    if [ "$(id -u)" -eq 0 ]; then command docker "$@"; else sudo docker "$@"; fi
}
compose() {
    docker_cmd compose --env-file "$IMT_ENV_FILE" -f "$DIR/compose.production.yaml" "$@"
}
case "${1:-}" in
    prepare)
        pg_image=$(compose config --format json | python3 -c 'import json,sys; print(json.load(sys.stdin)["services"]["postgres"]["image"])')
        secret_dir=$(compose config --format json | python3 -c 'import json,sys,pathlib; print(pathlib.Path(json.load(sys.stdin)["secrets"]["app_database_url"]["file"]).parent)')
        pg_uid=$(docker_cmd run --rm --network none --entrypoint id "$pg_image" -u postgres)
        sudo python3 "$DIR/prepare-secrets.py" --directory "$secret_dir" --postgres-uid "$pg_uid" --operator-uid "$(id -u)" --operator-gid "$(id -g)"
        ;;
    migrate)
        compose up -d --wait postgres
        compose --profile operations run --rm migrator
        compose exec -T postgres psql -q -v ON_ERROR_STOP=1 -U imt_owner -d imt_smart_teens -c 'REVOKE ALL ON TABLE _sqlx_migrations FROM imt_app'
        ;;
    initialize-empty)
        compose exec -T postgres psql -q -v ON_ERROR_STOP=1 -U imt_owner -d imt_smart_teens -f /opt/imt/initialize-empty-content.sql
        ;;
    up) compose up -d --wait postgres backend frontend ;;
    status) compose ps ;;
    backup)
        secret_dir=$(compose config --format json | python3 -c 'import json,sys,pathlib; print(pathlib.Path(json.load(sys.stdin)["secrets"]["app_database_url"]["file"]).parent)')
        backup_dir=$(dirname "$secret_dir")/backups
        mkdir -p "$backup_dir"
        chmod 700 "$backup_dir"
        target="$backup_dir/imt-$(date -u +%Y%m%dT%H%M%SZ).dump"
        [ ! -e "$target" ] || { printf '%s\n' 'Backup target exists; refusing overwrite' >&2; exit 1; }
        compose exec -T postgres pg_dump -U imt_owner -d imt_smart_teens -Fc > "$target"
        printf 'Private backup created: %s\n' "$target"
        ;;
    *) printf '%s\n' 'Usage: IMT_ENV_FILE=/private/compose.env operations.sh prepare|migrate|initialize-empty|up|status|backup' >&2; exit 2 ;;
esac

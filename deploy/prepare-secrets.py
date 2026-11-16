#!/usr/bin/env python3
"""Explicit root/operator-only secret preparation; never rotate an existing set."""
import argparse
import json
import os
from pathlib import Path
import secrets
import stat
import tempfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--postgres-uid", type=int, required=True)
    parser.add_argument("--operator-uid", type=int, required=True)
    parser.add_argument("--operator-gid", type=int, required=True)
    args = parser.parse_args()
    if os.geteuid() != 0:
        raise SystemExit("Run secret preparation through the authorized operator/root account")
    base = args.directory.resolve()
    if base == Path("/") or args.directory.is_symlink():
        raise SystemExit("Invalid secret directory")
    base.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    base.parent.chmod(0o700)
    os.chown(base.parent, args.operator_uid, args.operator_gid)
    ownership = {"postgres_password": args.postgres_uid, "app_password": args.postgres_uid,
                 "migrator_password": args.postgres_uid, "app_database_url": 10001,
                 "migration_database_url": 10001, "csrf_signing_key": 10001}
    if base.exists():
        if not base.is_dir() or set(p.name for p in base.iterdir()) != set(ownership) | {"metadata.json"}:
            raise SystemExit("Existing secret set is incomplete/unknown; refusing regeneration")
        if stat.S_IMODE(base.stat().st_mode) != 0o700:
            raise SystemExit("Existing secret directory permissions require operator review")
        for name, uid in ownership.items():
            p = base / name
            if p.is_symlink() or not p.is_file() or p.stat().st_size == 0 or p.stat().st_uid != uid or stat.S_IMODE(p.stat().st_mode) != 0o400:
                raise SystemExit("Existing secret ownership/permissions require operator review")
        print("Existing private secret set preserved; no values changed")
        return
    # Publish the complete set atomically; a partial initialization never becomes active.
    staging = Path(tempfile.mkdtemp(prefix=".secret-init-", dir=base.parent))
    owner_password, app_password, migrator_password, csrf = [secrets.token_hex(32) for _ in range(4)]
    values = {
        "postgres_password": owner_password,
        "app_password": app_password,
        "migrator_password": migrator_password,
        "app_database_url": f"postgresql://imt_app:{app_password}@postgres:5432/imt_smart_teens",
        "migration_database_url": f"postgresql://imt_migrator:{migrator_password}@postgres:5432/imt_smart_teens",
        "csrf_signing_key": csrf,
    }
    for name, value in values.items():
        p = staging / name
        fd = os.open(p, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o400)
        with os.fdopen(fd, "w") as stream:
            stream.write(value + "\n")
        os.chown(p, ownership[name], ownership[name])
    metadata = staging / "metadata.json"
    metadata.write_text(json.dumps({"schema": 1, "postgres_uid": args.postgres_uid, "backend_uid": 10001}) + "\n")
    metadata.chmod(0o600)
    os.chown(metadata, args.operator_uid, args.operator_gid)
    os.chown(staging, args.operator_uid, args.operator_gid)
    os.rename(staging, base)
    print("Fresh private secret set created; no values displayed")


if __name__ == "__main__":
    main()

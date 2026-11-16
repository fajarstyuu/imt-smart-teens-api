#!/usr/bin/env python3
"""Explicit root/operator-only secret preparation; never repair arbitrary paths."""
import argparse
import json
import os
from pathlib import Path
import pwd
import secrets
import stat
import tempfile

BACKEND_UID = 10001


def validate_location(directory, operator_uid, operator_gid):
    """Validate the complete lexical and existing-parent boundary before writes."""
    try:
        account = pwd.getpwuid(operator_uid)
        home = Path(account.pw_dir)
    except KeyError:
        raise SystemExit("Unknown operator account") from None
    if account.pw_gid != operator_gid:
        raise SystemExit("Operator group does not match the intended account")
    app = home / ".config/imt-smart-teens"
    if (not home.is_absolute() or not directory.is_absolute() or ".." in directory.parts
            or directory.name != "secrets" or directory.parent not in [app / "production", app / "smoke"]):
        raise SystemExit("Secrets require the operator's dedicated IMT production/smoke directory")
    # Do not resolve symlinks into some other owner's or shared system directory.
    if any(p.is_symlink() for p in [directory, *directory.parents]):
        raise SystemExit("Symlink ancestry is not allowed for private secret preparation")
    if not home.is_dir() or home.stat().st_uid != operator_uid or home.stat().st_mode & 0o022:
        raise SystemExit("Operator home ownership/permissions require review")
    parents = [home / ".config", app, directory.parent]
    for parent in parents:
        if not parent.exists():
            continue
        if not parent.is_dir() or parent.stat().st_uid != operator_uid:
            raise SystemExit("Existing private parent ownership/type requires operator review")
        mode = stat.S_IMODE(parent.stat().st_mode)
        # Shared .config/namespace directories may retain the trusted operator's
        # primary-group access. The actual production/smoke private parent is700.
        unsafe_shared = mode & 0o002 or (mode & 0o020 and parent.stat().st_gid != operator_gid)
        if (parent == directory.parent and mode != 0o700) or unsafe_shared:
            raise SystemExit("Existing private parent permissions require explicit operator review")
    return parents


def validate_existing(base, ownership, operator_uid, postgres_uid):
    if not base.is_dir() or set(p.name for p in base.iterdir()) != set(ownership) | {"metadata.json"}:
        raise SystemExit("Existing secret set is incomplete/unknown; refusing regeneration")
    if base.stat().st_uid != operator_uid or stat.S_IMODE(base.stat().st_mode) != 0o700:
        raise SystemExit("Existing secret directory ownership/permissions require operator review")
    for name, uid in ownership.items():
        p = base / name
        if p.is_symlink() or not p.is_file() or p.stat().st_size == 0 or p.stat().st_uid != uid or stat.S_IMODE(p.stat().st_mode) != 0o400:
            raise SystemExit("Existing secret ownership/permissions require operator review")
    metadata = base / "metadata.json"
    if (metadata.is_symlink() or not metadata.is_file() or metadata.stat().st_uid != operator_uid
            or stat.S_IMODE(metadata.stat().st_mode) != 0o600 or metadata.stat().st_size > 4096):
        raise SystemExit("Existing secret metadata requires operator review")
    try:
        value = json.loads(metadata.read_text())
    except (ValueError, OSError):
        raise SystemExit("Invalid existing secret metadata") from None
    if value != {"schema": 1, "postgres_uid": postgres_uid, "backend_uid": BACKEND_UID}:
        raise SystemExit("Existing secret metadata does not match the intended service identities")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--postgres-uid", type=int, required=True)
    parser.add_argument("--operator-uid", type=int, required=True)
    parser.add_argument("--operator-gid", type=int, required=True)
    args = parser.parse_args()
    if os.geteuid() != 0:
        raise SystemExit("Run secret preparation through the authorized operator/root account")
    if any(uid < 0 or uid > 0x7fffffff for uid in [args.postgres_uid, args.operator_uid, args.operator_gid]):
        raise SystemExit("Invalid service/operator identity")
    base = args.directory
    parents = validate_location(base, args.operator_uid, args.operator_gid)
    ownership = {"postgres_password": args.postgres_uid, "app_password": args.postgres_uid,
                 "migrator_password": args.postgres_uid, "app_database_url": BACKEND_UID,
                 "migration_database_url": BACKEND_UID, "csrf_signing_key": BACKEND_UID}
    if base.exists():
        validate_existing(base, ownership, args.operator_uid, args.postgres_uid)
        print("Existing private secret set preserved; no values or permissions changed")
        return
    # Every rejection above is read-only. Only newly created dedicated directories
    # may be assigned ownership; an existing .config or private parent is untouched.
    for parent in parents:
        if not parent.exists():
            parent.mkdir(mode=0o700)
            os.chown(parent, args.operator_uid, args.operator_gid)
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
    metadata.write_text(json.dumps({"schema": 1, "postgres_uid": args.postgres_uid, "backend_uid": BACKEND_UID}) + "\n")
    metadata.chmod(0o600)
    os.chown(metadata, args.operator_uid, args.operator_gid)
    os.chown(staging, args.operator_uid, args.operator_gid)
    os.rename(staging, base)
    print("Fresh private secret set created; no values displayed")


if __name__ == "__main__":
    main()

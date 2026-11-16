#!/usr/bin/env python3
"""Restore only an owned synthetic backup into a new smoke-only probe database."""
from pathlib import Path
import argparse
import json
import os
import stat
import subprocess
import uuid


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--backup-file", type=Path, required=True)
    args = parser.parse_args()
    path = args.backup_file.resolve()
    assert path.parent.name == "backups" and path.parent.parent.name == "smoke", "Only smoke backups allowed"
    assert stat.S_IMODE(path.stat().st_mode) == 0o600, "Backup must be private"
    container = "imt-production-smoke-postgres-1"
    project = subprocess.check_output(["sudo", "docker", "inspect", container, "--format", '{{index .Config.Labels "com.docker.compose.project"}}'], text=True).strip()
    assert project == "imt-production-smoke"
    def run(*argv, **kw):
        return subprocess.run(["sudo", "docker", "exec", "-i", container, *argv], check=True, capture_output=True, **kw).stdout
    catalog = run("pg_restore", "--list", input=path.read_bytes())
    assert b"identity_schools" in catalog and b"_sqlx_migrations" in catalog
    database = "imt_restore_probe_" + uuid.uuid4().hex[:12]
    run("createdb", "-U", "imt_owner", "--template=template0", database)
    try:
        run("pg_restore", "--exit-on-error", "-U", "imt_owner", "-d", database, input=path.read_bytes())
        counts = run("psql", "-qAt", "-U", "imt_owner", "-d", database, "-c", "SELECT (SELECT count(*) FROM identity_schools),(SELECT count(*) FROM _sqlx_migrations),(SELECT count(*) FROM identity_admins),(SELECT count(*) FROM identity_students)").decode().strip().split("|")
        assert int(counts[0]) > 0 and counts[1:] == ["5", "0", "0"]
        print("PASS: private dump catalog and real restore retain synthetic school data/five migrations; no admin/student fixtures")
    finally:
        run("dropdb", "-U", "imt_owner", database)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Real, explicitly smoke-only container verification; no authentication values printed."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import urllib.request
import urllib.error
import uuid


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--env-file", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    compose = ["sudo", "docker", "compose", "--env-file", str(args.env_file), "-f", str(root / "deploy/compose.production.yaml")]
    def command(argv, **kwargs):
        return subprocess.run(argv, capture_output=True, text=True, check=True, **kwargs).stdout.strip()
    config = json.loads(command(compose + ["config", "--format", "json"]))
    assert config["name"].endswith("-smoke"), "Refusing non-smoke project"
    secret_path = Path(config["secrets"]["app_database_url"]["file"])
    assert secret_path.parent.parent.name == "smoke", "Refusing non-smoke secrets"
    port = config["services"]["frontend"]["ports"][0]["published"]
    base = f"http://127.0.0.1:{port}"
    def sql(statement, expected_error=False):
        result = subprocess.run(compose + ["exec", "-T", "postgres", "psql", "-qAt", "-v", "ON_ERROR_STOP=1", "-v", "VERBOSITY=sqlstate", "-U", "imt_owner", "-d", "imt_smart_teens", "-c", statement], capture_output=True, text=True)
        if expected_error:
            assert result.returncode and "42501" in result.stderr, "Expected real insufficient-privilege denial"
            return "denied"
        assert result.returncode == 0, "Private smoke SQL failed; no query/value diagnostics printed"
        return result.stdout.strip()
    def request(path, method="GET", origin="https://imtsmartteens.my.id", ip="198.51.100.110", body=None, cookie=None, csrf=None):
        headers = {"Origin": origin, "X-Real-IP": ip, "Content-Type": "application/json"}
        if cookie: headers["Cookie"] = cookie
        if csrf: headers["X-CSRF-Token"] = csrf
        req = urllib.request.Request(base + path, data=json.dumps(body).encode() if body is not None else None, headers=headers, method=method)
        try: response = urllib.request.urlopen(req, timeout=10)
        except urllib.error.HTTPError as error: response = error
        return response.status, response.headers, response.read()
    assert request("/healthz")[0] == 200
    status, _, html = request("/")
    assert status == 200 and b"/assets/app-" in html and b"nginx!" not in html
    assert request("/admin/login")[0] == 200
    assert request("/api/v1/config")[0] == 200
    status, headers, payload = request("/api/v1/csrf")
    assert status == 200 and headers.get("Access-Control-Allow-Origin") == "https://imtsmartteens.my.id"
    assert headers.get("Access-Control-Allow-Credentials") == "true"
    cookies = headers.get_all("Set-Cookie")
    assert cookies and all("Secure" in c and "HttpOnly" in c and "SameSite=Lax" in c for c in cookies)
    assert all("Domain=" not in c for c in cookies)
    cookie = "; ".join(c.split(";", 1)[0] for c in cookies)
    csrf = json.loads(payload)["data"]["token"]
    invalid_login = {"email": "smoke-nonexistent@example.invalid", "password": "synthetic-no-account-password"}
    assert request("/api/v1/admin/sessions", "POST", body=invalid_login, cookie=cookie, csrf=csrf)[0] == 401
    status, headers, _ = request("/api/v1/admin/sessions", "POST", origin="https://evil.invalid", body=invalid_login, cookie=cookie, csrf=csrf)
    assert status == 403 and headers.get("Access-Control-Allow-Origin") is None
    # Request through the actual frontend socket, preserving distinct trusted client IPs.
    for _ in range(10):
        assert request("/api/v1/admin/sessions", "POST", ip="198.51.100.111", body=invalid_login, cookie=cookie, csrf=csrf)[0] == 401
    assert request("/api/v1/admin/sessions", "POST", ip="198.51.100.111", body=invalid_login, cookie=cookie, csrf=csrf)[0] == 429
    assert request("/api/v1/admin/sessions", "POST", ip="2001:db8::111", body=invalid_login, cookie=cookie, csrf=csrf)[0] == 401
    for statement in ["CREATE TABLE forbidden_probe(id int)", "ALTER TABLE identity_schools ADD COLUMN forbidden_probe text", "UPDATE _sqlx_migrations SET installed_on=installed_on"]:
        sql("BEGIN; SET LOCAL ROLE imt_app; " + statement + "; ROLLBACK", expected_error=True)
    assert sql("SELECT count(*) FROM assessment_banks") == "2"
    assert sql("SELECT count(*) FROM learning_path_drafts") == "1"
    assert sql("SELECT count(*) FROM identity_admins") == "0"
    assert sql("SELECT count(*) FROM identity_students") == "0"
    probe = str(uuid.uuid4())
    sql(f"BEGIN; SET LOCAL ROLE imt_app; INSERT INTO identity_schools(id,name,normalized_name) VALUES('{probe}','Synthetic container persistence probe','{probe}'); COMMIT")
    def hashes():
        # Compare inside this process only; even credential hashes are not printed.
        return command(compose + ["exec", "-T", "backend", "sha256sum", "/run/secrets/app_database_url", "/run/secrets/csrf_signing_key"])
    before = hashes()
    command(compose + ["up", "-d", "--force-recreate", "--wait", "postgres", "backend", "frontend"], timeout=120)
    assert hashes() == before
    assert sql(f"SELECT count(*) FROM identity_schools WHERE id='{probe}'") == "1"
    assert request("/api/v1/schools")[0] == 200
    for name in ["backend", "postgres"]:
        container = command(compose + ["ps", "-q", name])
        ports = json.loads(command(["sudo", "docker", "inspect", container, "--format", "{{json .NetworkSettings.Ports}} "]))
        assert all(not bindings for bindings in ports.values()), "Private service published a port"
    print("PASS: real three-service health, built SPA/API, secure host-only cookies, exact CORS/CSRF, distinct IPv4/IPv6 rate buckets, actual DDL/migration-ledger denial, empty roots, and data/secret persistence through recreation")


if __name__ == "__main__":
    main()

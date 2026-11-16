"""Production boundaries: configuration checks complement real container smoke tests."""
import json
import os
from pathlib import Path
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ProductionContract(unittest.TestCase):
    def config(self):
        path = ROOT / "deploy/compose.production.yaml"
        self.assertTrue(path.is_file(), "production Compose artifact is missing")
        env = dict(os.environ, IMT_SECRET_DIR="/nonexistent/synthetic-contract-secrets",
                   FRONTEND_BUILD_CONTEXT="/nonexistent/synthetic-frontend")
        result = subprocess.run(["docker", "compose", "--profile", "operations", "-f", str(path), "config", "--format", "json"],
                                env=env, capture_output=True, text=True, check=True)
        return json.loads(result.stdout)

    def test_only_frontend_publishes_loopback(self):
        c = self.config()
        for name in ["backend", "postgres", "migrator"]:
            self.assertFalse(c["services"][name].get("ports"))
        ports = c["services"]["frontend"]["ports"]
        self.assertEqual(len(ports), 1)
        self.assertEqual(ports[0]["host_ip"], "127.0.0.1")
        self.assertEqual(str(ports[0]["published"]), "5180")

    def test_production_security_is_exact_and_no_runtime_ddl_secret(self):
        c = self.config()
        backend = c["services"]["backend"]
        e = backend["environment"]
        self.assertEqual(e["APP_ENV"], "production")
        self.assertEqual(e["SESSION_COOKIE_SECURE"], "true")
        self.assertEqual(e["CORS_ALLOWED_ORIGINS"], "https://imtsmartteens.my.id")
        self.assertEqual(e["TRUSTED_PROXY_IPS"], "172.30.251.2")
        self.assertFalse(any(k in e for k in ["DATABASE_URL", "MIGRATION_DATABASE_URL", "CSRF_SIGNING_KEY"]))
        self.assertEqual({s["source"] for s in backend["secrets"]}, {"app_database_url", "csrf_signing_key"})
        self.assertEqual({s["source"] for s in c["services"]["migrator"]["secrets"]}, {"migration_database_url"})

    def test_networks_runtime_hardening_and_explicit_operations(self):
        c = self.config()
        self.assertTrue(c["networks"]["app"]["internal"])
        self.assertTrue(c["networks"]["database"]["internal"])
        self.assertIn("edge", c["networks"], "published frontend needs an ingress-capable network")
        self.assertFalse(c["networks"]["edge"].get("internal", False))
        self.assertIn("edge", c["services"]["frontend"]["networks"])
        self.assertNotIn("edge", c["services"]["backend"]["networks"])
        self.assertNotIn("database", c["services"]["frontend"]["networks"])
        self.assertNotIn("app", c["services"]["postgres"]["networks"])
        self.assertEqual(c["services"]["frontend"]["networks"]["app"]["ipv4_address"], "172.30.251.2")
        for name in ["backend", "frontend"]:
            service = c["services"][name]
            self.assertTrue(service["read_only"])
            self.assertEqual(service["tmpfs"], ["/tmp:rw,noexec,nosuid,size=32m,mode=1777"])
            self.assertIn("ALL", service["cap_drop"])
            self.assertTrue(any(s.startswith("no-new-privileges") for s in service["security_opt"]))
            self.assertTrue(service["healthcheck"])
        self.assertEqual(c["services"]["migrator"]["profiles"], ["operations"])
        self.assertEqual(c["services"]["migrator"]["command"], ["migrate"])
        self.assertIn("postgres_data", c["volumes"])

    def test_build_context_excludes_private_and_generated_files(self):
        p = ROOT / ".dockerignore"
        self.assertTrue(p.is_file(), "backend Docker build exclusions missing")
        text = p.read_text()
        for value in [".env", ".git", ".worktrees", ".superpowers", "target"]:
            self.assertIn(value, text)
        p = ROOT / "Dockerfile"
        self.assertTrue(p.is_file(), "backend image missing")
        text = p.read_text()
        self.assertRegex(text, r"FROM rust:1\.100\.0-bookworm@sha256:[a-f0-9]{64}")
        self.assertIn("--locked", text)
        self.assertRegex(text, r"(?m)^USER [1-9][0-9]*")
        self.assertNotIn("COPY .env", text)

    def test_private_bootstrap_preserves_production_boundaries(self):
        role = ROOT / "deploy/postgres/init-roles.sh"
        sql = ROOT / "deploy/postgres/initialize-empty-content.sql"
        self.assertTrue(role.is_file(), "private role bootstrap missing")
        self.assertTrue(sql.is_file(), "explicit root initialization missing")
        text = role.read_text()
        for required in ["NOSUPERUSER", "NOCREATEDB", "NOCREATEROLE", "ALTER DEFAULT PRIVILEGES", "REVOKE CREATE"]:
            self.assertIn(required, text)
        text = sql.read_text()
        self.assertIn("pg_advisory_xact_lock(718304291)", text)
        self.assertIn("ON CONFLICT", text)
        self.assertNotRegex(text.upper(), r"\b(?:DELETE|TRUNCATE|DROP)\b")
        self.assertNotIn("identity_admins", text)
        self.assertNotIn("identity_students", text)


if __name__ == "__main__":
    unittest.main()

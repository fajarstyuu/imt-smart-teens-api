"""Validate local PostgreSQL Compose configuration without a Docker daemon."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

class ComposeContract(unittest.TestCase):
    def config(self, password="contract-only-not-a-real-secret", port="55432"):
        # Isolate interpolation from a developer's real shell and .env.
        env = {k: v for k, v in os.environ.items()
               if not k.startswith("POSTGRES_") and k != "COMPOSE_FILE"}
        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory) / "contract.env"
            fixture.write_text(
                f"POSTGRES_DB=imt_smart_teens_dev\nPOSTGRES_USER=imt_dev\n"
                f"POSTGRES_PASSWORD={password}\nPOSTGRES_PORT={port}\n")
            return subprocess.run(
                ["docker", "compose", "--env-file", str(fixture),
                 "-f", str(ROOT / "compose.yaml"), "config", "--format", "json"],
                env=env, text=True, capture_output=True)

    def test_local_persistent_password_required(self):
        result = self.config()
        self.assertEqual(result.returncode, 0, "Compose config failed; inspect locally without publishing secrets")
        config = json.loads(result.stdout)
        self.assertEqual(set(config["services"]), {"postgres"})
        service = config["services"]["postgres"]
        self.assertEqual(service["image"], "postgres:17-bookworm")
        self.assertEqual(service["ports"][0]["host_ip"], "127.0.0.1")
        self.assertEqual(str(service["ports"][0]["published"]), "55432")
        self.assertEqual(service["ports"][0]["target"], 5432)
        self.assertEqual(service["environment"]["POSTGRES_INITDB_ARGS"], "--auth-host=scram-sha-256")
        self.assertNotIn("POSTGRES_HOST_AUTH_METHOD", service["environment"])
        self.assertTrue(any(v["type"] == "volume" and v["source"] == "postgres_data"
                            and v["target"] == "/var/lib/postgresql/data"
                            for v in service["volumes"]))
        self.assertIn("postgres_data", config["volumes"])
        self.assertEqual(service["restart"], "unless-stopped")
        self.assertIn("pg_isready", " ".join(service["healthcheck"]["test"]))
        self.assertNotIn("container_name", service)

    def test_empty_password_rejected(self):
        result = self.config(password="")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("POSTGRES_PASSWORD", result.stderr)

    def test_port_override_remains_private(self):
        result = self.config(port="55433")
        self.assertEqual(result.returncode, 0, "Compose config failed")
        port = json.loads(result.stdout)["services"]["postgres"]["ports"][0]
        self.assertEqual(port["host_ip"], "127.0.0.1")
        self.assertEqual(str(port["published"]), "55433")

if __name__ == "__main__":
    unittest.main()

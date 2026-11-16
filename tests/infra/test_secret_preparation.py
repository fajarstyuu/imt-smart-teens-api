"""Privileged preparation is tested without root or system-directory mutations."""
from contextlib import ExitStack, redirect_stdout
import importlib.util
import io
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("imt_prepare_secrets", ROOT / "deploy/prepare-secrets.py")
PREPARE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PREPARE)


class SecretPreparationSafety(unittest.TestCase):
    def setUp(self):
        scratch = Path(os.environ.get("TMPDIR", "/home/ubuntu/.hermes/cache/scratch"))
        self.temp = tempfile.TemporaryDirectory(prefix="imt-secret-safety-", dir=scratch)
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name) / "operator"
        self.home.mkdir(mode=0o700)
        self.uid, self.gid = os.getuid(), os.getgid()

    def arguments(self, directory):
        return ["prepare-secrets.py", "--directory", str(directory), "--postgres-uid", str(self.uid),
                "--operator-uid", str(self.uid), "--operator-gid", str(self.gid)]

    def assert_rejected_without_mutation(self, directory):
        # Every mutator, including staging publication, is intercepted. Unsafe path
        # strings can never touch /, /etc or any other real system directory.
        with ExitStack() as stack:
            stack.enter_context(patch("sys.argv", self.arguments(directory)))
            stack.enter_context(patch.object(PREPARE.os, "geteuid", return_value=0))
            stack.enter_context(patch("pwd.getpwuid", return_value=SimpleNamespace(pw_dir=str(self.home), pw_gid=self.gid)))
            mkdir = stack.enter_context(patch.object(Path, "mkdir"))
            chmod = stack.enter_context(patch.object(Path, "chmod"))
            chown = stack.enter_context(patch.object(PREPARE.os, "chown"))
            rename = stack.enter_context(patch.object(PREPARE.os, "rename"))
            staging = stack.enter_context(patch.object(PREPARE.tempfile, "mkdtemp", side_effect=SystemExit("staging intercepted")))
            with self.assertRaises(SystemExit):
                PREPARE.main()
            self.assertEqual(mkdir.call_count, 0, "unsafe target attempted directory creation")
            self.assertEqual(chmod.call_count, 0, "unsafe target attempted chmod")
            self.assertEqual(chown.call_count, 0, "unsafe target attempted chown")
            self.assertEqual(staging.call_count, 0, "unsafe target attempted secret generation")
            self.assertEqual(rename.call_count, 0, "unsafe target attempted publication")

    def test_shared_and_unapproved_paths_rejected_before_any_mutator(self):
        for path in [Path("/secrets"), Path("/etc/imt-smart-teens"), self.home / "secrets",
                     self.home / ".config/imt-smart-teens/other/secrets"]:
            with self.subTest(path=str(path)):
                self.assert_rejected_without_mutation(path)

    def test_partial_existing_set_rejected_without_changing_parent(self):
        target = self.home / ".config/imt-smart-teens/production/secrets"
        target.mkdir(parents=True, mode=0o700)
        target.parent.chmod(0o700)
        target.parent.parent.chmod(0o700)
        (target / "unknown-file").write_text("synthetic incomplete set")
        self.assert_rejected_without_mutation(target)

    def test_symlinked_ancestry_rejected_without_touching_shared_target(self):
        shared = Path(self.temp.name) / "shared"
        shared.mkdir()
        (self.home / ".config").symlink_to(shared, target_is_directory=True)
        self.assert_rejected_without_mutation(self.home / ".config/imt-smart-teens/production/secrets")

    def test_existing_parent_permissions_are_not_silently_repaired(self):
        parent = self.home / ".config/imt-smart-teens/production"
        parent.mkdir(parents=True, mode=0o755)
        parent.parent.chmod(0o700)
        self.assert_rejected_without_mutation(parent / "secrets")

    def test_fresh_dedicated_set_and_existing_set_preservation(self):
        target = self.home / ".config/imt-smart-teens/production/secrets"
        config = self.home / ".config"
        config.mkdir(mode=0o775)
        config.chmod(0o775)
        namespace = config / "imt-smart-teens"
        namespace.mkdir(mode=0o775)
        namespace.chmod(0o775)
        with ExitStack() as stack:
            stack.enter_context(patch("sys.argv", self.arguments(target)))
            stack.enter_context(patch.object(PREPARE.os, "geteuid", return_value=0))
            stack.enter_context(patch("pwd.getpwuid", return_value=SimpleNamespace(pw_dir=str(self.home), pw_gid=self.gid)))
            stack.enter_context(patch.object(PREPARE, "BACKEND_UID", self.uid, create=True))
            chown = stack.enter_context(patch.object(PREPARE.os, "chown"))
            stack.enter_context(redirect_stdout(io.StringIO()))
            PREPARE.main()
            self.assertEqual(config.stat().st_mode & 0o777, 0o775)
            self.assertEqual(namespace.stat().st_mode & 0o777, 0o775)
            self.assertTrue(all(call.args[0] not in [config, namespace] for call in chown.call_args_list))
            self.assertEqual(target.stat().st_mode & 0o777, 0o700)
            self.assertEqual(len(list(target.iterdir())), 7)
            snapshot = {p.name: (p.read_bytes(), p.stat().st_mode, p.stat().st_mtime_ns) for p in target.iterdir()}
            chown.reset_mock()
            with patch.object(Path, "mkdir") as mkdir, patch.object(Path, "chmod") as chmod, patch.object(PREPARE.tempfile, "mkdtemp") as generate, patch.object(PREPARE.os, "rename") as publish:
                PREPARE.main()
                self.assertFalse(mkdir.called or chmod.called or chown.called or generate.called or publish.called)
            after = {p.name: (p.read_bytes(), p.stat().st_mode, p.stat().st_mtime_ns) for p in target.iterdir()}
            self.assertTrue(snapshot == after, "existing secrets or metadata were changed")


if __name__ == "__main__":
    unittest.main()

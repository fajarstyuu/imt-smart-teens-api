"""Isolated release-boundary regressions; no Docker, private inputs or live filesystem mutations."""
from pathlib import Path
import importlib.util
import sys
import unittest

ROOT=Path(__file__).resolve().parents[2]
# A missing guard is a failing release boundary, never a reason to touch production.
GUARD=ROOT/'deploy/release_guard.py'

class ReleaseGuardTests(unittest.TestCase):
    def load(self):
        self.assertTrue(GUARD.exists(),'An explicit reviewed-release guard must exist')
        spec=importlib.util.spec_from_file_location('isolated_release_guard',GUARD)
        module=importlib.util.module_from_spec(spec)
        sys.modules[spec.name]=module
        spec.loader.exec_module(module)
        return module

    def target(self):
        return {'backend':{'sha':'a'*40,'image_id':'sha256:'+'b'*64},'frontend':{'sha':'c'*40,'image_id':'sha256:'+'d'*64}}

    def test_unreviewed_clean_head_or_moved_tag_stops_before_mutation(self):
        guard=self.load();manifest=self.target()
        for drift in ['head','tag','dirty']:
            writes=[]
            with self.assertRaises(guard.ReleaseMismatch):
                guard.verify_release(manifest,
                    lambda service:'e'*40 if drift=='head' and service=='backend' else manifest[service]['sha'],
                    lambda service:'sha256:'+'f'*64 if drift=='tag' and service=='frontend' else manifest[service]['image_id'],
                    lambda service:drift!='dirty')
                writes.append('first-production-write')
            self.assertEqual(writes,[])
        guard.verify_release(manifest,lambda s:manifest[s]['sha'],lambda s:manifest[s]['image_id'],lambda s:True)

    def rollback(self,fail_action=None,fail_verification=None):
        guard=self.load();expected=self.target();calls=[]
        def checkout(service):
            calls.append('checkout:'+service)
            if fail_action=='checkout' and service=='backend':raise OSError('synthetic failure')
        def settings():
            calls.append('settings')
            if fail_action=='settings':raise OSError('synthetic failure')
        def images():
            calls.append('images')
            if fail_action=='images':raise OSError('synthetic failure')
        result=guard.restore_and_verify(expected,checkout,settings,images,
            lambda s:'e'*40 if fail_verification=='head' and s=='backend' else expected[s]['sha'],
            lambda s:'sha256:'+'f'*64 if fail_verification=='image' and s=='frontend' else expected[s]['image_id'],
            lambda s:fail_verification!='health',lambda:fail_verification!='settings')
        return result,calls

    def test_checkout_restore_failure_is_reported_and_other_actions_still_run(self):
        result,calls=self.rollback(fail_action='checkout')
        self.assertFalse(result.verified)
        self.assertIn('restore-checkout:backend',result.failures)
        self.assertEqual(calls,['checkout:backend','checkout:frontend','settings','images'])

    def test_compose_restore_failure_is_not_reported_as_restored(self):
        result,calls=self.rollback(fail_action='images')
        self.assertFalse(result.verified)
        self.assertIn('restore-images',result.failures)
        self.assertIn('settings',calls)

    def test_each_readback_failure_blocks_verified_rollback(self):
        for drift in ['head','image','health','settings']:
            result,_=self.rollback(fail_verification=drift)
            self.assertFalse(result.verified,drift)
            self.assertTrue(result.failures)

    def test_exact_old_state_is_verified_only_after_all_restores_and_readbacks(self):
        result,calls=self.rollback()
        self.assertTrue(result.verified)
        self.assertEqual(result.failures,())
        self.assertEqual(len(calls),4)

if __name__=='__main__':unittest.main()

"""Fail-closed release identity and independently verified rollback boundaries.

Adapters supply checked I/O callbacks. This module never reads credentials, invokes
Docker, or formats arbitrary exception details into operator diagnostics.
"""
from dataclasses import dataclass
import re


class ReleaseMismatch(RuntimeError):
    """A candidate differs from an explicitly approved source/image identity."""


def verify_release(manifest, read_head, read_tag_image, is_clean):
    """Reject drift before callers make their first checkout/settings/service write."""
    if set(manifest) != {'backend', 'frontend'}:
        raise ReleaseMismatch('Release must name backend and frontend explicitly')
    for service, target in manifest.items():
        if not re.fullmatch(r'[0-9a-f]{40}', target.get('sha', '')):
            raise ReleaseMismatch('Invalid approved source identity: '+service)
        if not re.fullmatch(r'sha256:[0-9a-f]{64}', target.get('image_id', '')):
            raise ReleaseMismatch('Invalid approved image identity: '+service)
        if not is_clean(service):
            raise ReleaseMismatch('Candidate checkout is not clean: '+service)
        if read_head(service) != target['sha']:
            raise ReleaseMismatch('Candidate source differs from approved release: '+service)
        if read_tag_image(service) != target['image_id']:
            raise ReleaseMismatch('Candidate image tag differs from approved release: '+service)


@dataclass(frozen=True)
class RollbackResult:
    verified: bool
    failures: tuple[str, ...]


def restore_and_verify(expected, restore_checkout, restore_settings, restore_images,
                       read_head, read_image, is_ready, settings_match):
    """Try every restoration, then independently read every exact old target back.

    A failed action or readback produces an incomplete result, even if subsequent
    checks happen to succeed. Failure labels expose stages, never command stderr,
    private values or exception objects. Callers must not assert restoration on
    an incomplete result; preserve any newly created account for reconciliation.
    """
    failures=[]
    def attempt(label, action):
        try:
            action()
        except Exception:
            failures.append(label)
    for service in expected:
        attempt('restore-checkout:'+service, lambda s=service: restore_checkout(s))
    attempt('restore-settings', restore_settings)
    attempt('restore-images', restore_images)

    def check(label, predicate):
        try:
            valid=predicate()
        except Exception:
            valid=False
        if not valid:
            failures.append(label)
    for service, target in expected.items():
        check('verify-checkout:'+service, lambda s=service,t=target: read_head(s)==t['sha'])
        check('verify-image:'+service, lambda s=service,t=target: read_image(s)==t['image_id'])
        check('verify-readiness:'+service, lambda s=service: is_ready(s))
    check('verify-settings', settings_match)
    return RollbackResult(not failures, tuple(failures))

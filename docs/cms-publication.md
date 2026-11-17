# Reviewed CMS publication and report summary additions

The normal CMS uses one publication page; lower-level publication endpoints remain compatible for existing integrations.

## POST /api/v1/admin/learning-path/release
Requires an active global-admin session and normal session-bound CSRF/origin validation. Body (unknown fields rejected):

```json
{"expected_revision":4,"lessons":[{"id":"lesson UUID","expected_revision":3}],"banks":[{"id":"paired bank UUID","expected_revision":2},{"id":"quiz bank UUID","expected_revision":5}]}
```

IDs above are explanatory placeholders, not executable seed values. `lessons` must match the complete reviewed path order exactly. `banks` must contain exactly the configured paired and quiz roots, no extras/duplicates. Every expected revision is positive and checked under the transaction's path -> sorted banks -> sorted lessons root locks. All active question drafts and section/completion metadata are validated before immutable children/audits are persisted. Existing snapshot writers and path admission are reused under the same transaction. Any error rolls back every new version and audit.

Success returns HTTP200 with the existing PathEditor data envelope. A subsequent read of /admin/learning-path verifies the returned publication identity. Stale reviews return409, invalid or incomplete content422; missing authentication401 with valid CSRF, CSRF violations403. A timeout/lost response is not permission to replay: review the latest path/readiness explicitly. Expected path revision prevents duplicate release of the old review. Old enrollment/attempt snapshots do not change; only future enrollment chooses the new active path.

## GET /api/v1/admin/students

Adds optional `q` (literal case-insensitive name substring, max200 characters) to existing allowlisted filters, stable pagination and global active-admin scope. The existing StudentSummary DTO additionally returns `pre_test_score`, `post_test_score` (integer|null, submitted grading only; zero is a real score), and `latest_activity_at` (precise UTC timestamp from enrollment, attempts/answers, lesson activity and measurements). Projection occurs in the authorized bounded database read, not one browser request per student.

Storage timestamps retain full precision. Admin presentation uses Indonesian calendar dates and Asia/Jakarta timestamps to hour/minute, explicitly WIB. datetime-local filters are interpreted as WIB and converted to the existing UTC RFC3339 API; form dates remain timezone-independent calendar dates.

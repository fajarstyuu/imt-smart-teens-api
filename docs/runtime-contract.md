# Implemented student runtime contract

This documents the Rust v1 runtime, not the legacy Node fixtures. IDs below are UUID strings, timestamps RFC3339 UTC, calendar dates `YYYY-MM-DD`. Success wraps the listed payload in `{data: ...}`. Errors are `{error:{code,message,details?},request_id}` with Indonesian messages. Mutations require allowlisted `Origin`, the cookie-bound `X-CSRF-Token`, and a valid role-session binding where authenticated. Responses have `Cache-Control: no-store` and `X-Request-ID`. There are no legacy fixture aliases or browser bearer tokens.

## Browser/session and retry protocol

1. Call `GET /api/v1/csrf` with credentials included. It returns `{data:{token}}` and signed `imt_csrf` plus `imt_enrollment_context` HttpOnly cookies. Keep the token in memory, not a bearer-token store. Browsers retain cookies; do not manually construct Cookie headers in frontend code.
2. Enrollment requires `Idempotency-Key` (1–128 printable ASCII characters). Generate a random operation key once per deliberate enrollment action; retain it and the form through a retry. Send `{name,school_id,form_date}` to `POST /api/v1/student/sessions`. Success is 201 with `{student,enrollment,csrf_token}` and a new opaque `imt_student_session` plus rotated `imt_csrf`. Replace the in-memory CSRF token with `csrf_token`. Cookies use host-only scope, Path `/api/v1`, HttpOnly, SameSite=Lax, production Secure.
3. The separate signed enrollment-context cookie is established **before** enrollment and remains stable across CSRF/session rotations. Enrollment idempotency is scoped to its hash, not the newly rotated CSRF token. Both response loss (old cookies/token) and retry after receiving the response (rotated cookies/token) resolve the same enrollment when key and validated payload match. Name normalization (NFC/whitespace) precedes fingerprinting. A changed validated payload gives 409 `IDEMPOTENCY_CONFLICT`.
4. Enrollment replay issues a fresh random student session and CSRF token for the same bound browser operation. The response is not byte-identical because the fresh `csrf_token` changes; `student`/`enrollment` and status are the original cached result. A supplied previous student session is revoked in the same transaction. A coexisting live admin session is preserved and rebound to the new CSRF context, not revoked.
5. The cache stores **no raw session cookie/token or CSRF token**. The only enrollment replay JSON is the safe student/enrollment projection. The stable browser context is signed using the maintained CSRF implementation, expires after two hours and is reused, not extended, while valid. Operation records last 24 hours, but unauthenticated enrollment replay is available only while the browser still possesses that unexpired signed context and its operation key. Expiration/loss is not identity recovery: do not automatically retry an old enrollment with a newly issued browser context. There is no name-based or cross-device recovery.
6. Authenticated create-attempt, submission and measurement POSTs also require an operation key. Scope is enrollment + operation (submission additionally includes attempt ID). One transaction reserves the key, applies effects, stores the safe response and commits. A same-payload retry returns the original status/projection; changed payload gives 409. Numerically equivalent supported measurement spellings fingerprint identically. After 24-hour expiration a key is reusable; one-final-attempt semantics still prevent double submission. Idempotency replays may contain the stage/score visibility from the original response; refresh progress/attempt/summary for current state.
7. Refresh uses the student cookie. `GET /student/me` never identifies a student by name. GET csrf can refresh/rebind expired contexts for existing role sessions. Do not retry an uncertain enrollment as a new operation merely because an HTTP request timed out.

## Shared projections

```text
Student = {id, name}
Enrollment = {
  id, school_id, school_name_at_enrollment, form_date,
  path_version_id, created_at, completed_at: timestamp|null
}
LessonMetadata = {
  lesson_id, lesson_version_id, title, position,
  opened_at: timestamp|null, completed_at: timestamp|null
}
Progress = {
  enrollment_id, path_version_id,
  current_stage: pre_test|lessons|quiz|calculator|post_test|completed,
  stages: [{id: same stage enum, status: locked|in_progress|completed,
            locked_reason: "LEARNING_PREREQUISITE_NOT_MET"|null}],
  lessons: [LessonMetadata + {status: locked|not_started|in_progress|completed}],
  active_attempt_ids: {pre_test?: uuid, quiz?: uuid, post_test?: uuid},
  latest_measurement_id: uuid|null
}
Answer = {
  snapshot_question_id, option_ids: uuid[], revision: integer, saved_at: timestamp
}
Question = {
  id, question_id, question_version_id, position, text,
  type: single|multiple, required: boolean,
  options: [{id,text}]
}
Measurement = {
  id, weight_kg: number, height_cm: number, bmi: number,
  bmi_display: string, calculation_rule_version: "bmi_v1", created_at
}
```

`Question.id` is the **snapshot-question ID** used in answer URLs, not the mutable bank question ID. Option order follows immutable `position`; answer option IDs form a set and responses sort them by UUID. Questions have no keys, correctness, points or explanations, even after completion. `required` is public completion metadata. `score` is an integer percent or null, never implicitly zero.

## Implemented endpoints

All paths below start `/api/v1`.

| Method/path | Input | Success payload/status |
| --- | --- | --- |
| POST `/student/sessions` | `{name:string,school_id:uuid,form_date:string}` + key | 201 `{student:Student,enrollment:Enrollment,csrf_token:string}` |
| GET `/student/me` | cookie only | 200 `{student:Student,enrollment:Enrollment}` |
| DELETE `/student/sessions/current` | no body needed, valid CSRF | 204 empty; only student cookie revoked/cleared; repeat with valid CSRF also 204 |
| GET `/student/progress` | cookie only | 200 `Progress` |
| GET `/student/lessons/{lesson_id}` | pinned path logical lesson ID | 200 `LessonMetadata + {description,sections:[{id,title,body,required,position}],visited_section_ids:uuid[]}` |
| POST `/student/lessons/{lesson_id}/opened` | `{section_id?:uuid|null}` | 200 `{lesson_id,opened_at,visited_section_ids}` |
| POST `/student/lessons/{lesson_id}/completion` | `{}` | 200 `{lesson_id,completed_at,progress:Progress}` |
| POST `/student/attempts` | `{assessment_type:pre_test|quiz|post_test}` + key | 201 new, 200 existing `{attempt_id,assessment_type,attempt_number:1,status:in_progress|submitted,snapshot_id}` |
| GET `/student/attempts/{attempt_id}` | owned UUID | 200 `{id,assessment_type,attempt_number:1,status,snapshot_id,submitted_at:timestamp|null,score:integer|null,questions:Question[],answers:Answer[]}` |
| PUT `/student/attempts/{attempt_id}/answers/{snapshot_question_id}` | `{option_ids:uuid[],expected_revision:integer}` | 200 `Answer` |
| POST `/student/attempts/{attempt_id}/submission` | `{}` + key | 200 `{attempt_id,status:"submitted",submitted_at,score:integer|null,progress:Progress}` |
| POST `/student/measurements` | `{weight_kg:number,height_cm:number}` + key | 201 `Measurement + {progress:Progress}` |
| GET `/student/measurements?limit=25&offset=0` | limit 1–100; offset nonnegative | 200 `{data:Measurement[],meta:{limit,offset,total}}` (not double-wrapped) |
| GET `/student/summary` | cookie only | 200 `{student:Student,enrollment:Enrollment,scores:{pre_test:integer,quiz:integer,post_test:integer},latest_measurement:Measurement,progress:Progress}` |

Unknown command fields (including client scores/student IDs) are rejected with 422. Invalid path UUID or malformed registration school UUID is 400. Missing/unknown session is 401; role/CSRF/Origin denial is 403; unknown/not-owned attempt/path lesson is 404. Unknown option, duplicate option, wrong cardinality, foreign option or section is 422. Missing operation keys are 422 `INVALID_IDEMPOTENCY_KEY`.

## Prerequisites, saves and scores

- Enrollment locks the selected school row (the same row used by edit/archive), snapshots the school name, selects the newest immutable published path, pins both its paired and quiz banks and creates the pre-test plus browser session atomically. Unknown/archived school: 422 `SCHOOL_NOT_AVAILABLE`. Missing/empty path/bank: 409 `CONTENT_NOT_AVAILABLE`. Failure rolls back identity, enrollment, pins, attempts, session, cache and audit. Existing students continue after archive.
- Pre-test submission unlocks the first lesson. Each lesson requires all preceding pinned lessons completed. GET lesson bodies, opened/completion, quiz creation/read and post-test creation/read enforce prerequisites; metadata-only progress remains available. Failure: 409 `LEARNING_PREREQUISITE_NOT_MET`.
- Opening without `section_id` records opening only. Required sections must each be visited for completion; otherwise 409 `REQUIRED_SECTIONS_NOT_VISITED`. Opening/completion repeats preserve original timestamps. The server makes no timer/comprehension claims.
- Quiz requires all lessons; its submission unlocks calculator. At least one committed measurement unlocks post-test. New measurements are allowed only after quiz and before post-test submission. An idempotent replay of an earlier measurement remains safe after completion. Summary requires submitted post-test.
- Autosave locks the owned attempt. Absent answer revision is 0; a successful save increments by one. Empty `option_ids` clears while open and leaves an empty answer row with its new revision. Single choice accepts at most one option. Multiple choice accepts a subset of valid, distinct options. Saves after submission: 409 `ATTEMPT_SUBMITTED`.
- Stale save: 409 `ANSWER_REVISION_CONFLICT`, with `error.details.latest_answer` containing `Answer`. If no row yet exists it contains `{snapshot_question_id,option_ids:[],revision:0,saved_at:null}`. Preserve unsaved UI input; reconcile/retry deliberately with the returned revision, not a question-version number.
- Submission locks enrollment then attempt. Save takes only attempt and never subsequently enrollment, so it cannot invert that lock order. Required empty answers: 422 `REQUIRED_ANSWERS_MISSING`, no partial scores/progress. Save either commits before scoring or receives submitted conflict. One attempt/final submission per type, including repeats with a different operation key.
- Scoring uses the private pinned option set, exact set equality without partial/negative credit. Each question's immutable `max_points` is represented exactly at scale4 (normal publications use equal weight1). Percent rounds half-up to nearest integer; raw earned/max points and per-answer correctness are stored atomically. Quiz score is visible immediately; pre/post scores stay null until post-test submitted. Legitimate zero remains 0. Pre/post reference exactly the same snapshot question/option/key/order/rule versions; quiz has a separate snapshot.
- Published version rows are append-only; extension of already-referenced option/section/path/bank/snapshot containers is blocked by migration4/5 triggers. CMS draft roots stay editable. Changing drafts cannot rewrite a historical response.

## Decimal policy

Inputs must be JSON **numbers**, finite, positive, exactly representable as NUMERIC(12,4): smallest 0.0001 and largest 99999999.9999. Extra trailing zeros and equivalent exponent spelling are accepted; any necessary input rounding, overflow, zero or negative input is 422 `INVALID_MEASUREMENT`. Invalid JSON nonfinite literals are rejected at transport. No binary floating-point conversion occurs in the domain or driver adapter.

`bmi_v1 = weight_kg / (height_cm / 100)^2`, rounded half-up only at NUMERIC(38,12) storage precision; a result rounded to zero is rejected. Input scale4 and bounds fit checked i128 intermediate arithmetic exactly. `bmi_display` is a half-up one-decimal **string**, e.g. `"20.9"`, not a health category. Inputs and `bmi` serialize as JSON numbers. Clinical categories, healthy ranges and diagnoses are not returned.

## Internal typed results and operation-cache compatibility

Application/domain workflows expose typed progress, profile, lesson, attempt, submission and measurement results; HTTP DTO adapters alone select JSON field names, public enums and numeric tokens. Domain decimals accept/return checked exact lexical/scaled values, not JSON numbers. Stale-answer conflict facts are typed and mapped to the existing error.details.latest_answer wire shape by the HTTP error adapter. These internal changes preserve the public endpoint contracts above.

Operation persistence now uses a versioned infrastructure-only CacheV1 JSONB representation of safe semantic results, distinct from HTTP response bodies. It stores no raw cookies, session/CSRF tokens or answer keys. The existing relational status column is checked against the semantic result at replay. Unsupported/malformed versions and earlier unversioned records fail closed; they do not authorize repeating business writes. No transparent legacy-cache backfill is supplied by this predeployment change. A rollout over an older live version with existing operation records requires an explicit compatibility transition; final runtime integration remains a separate gate.

Both pinned assessment banks are checked against the aggregate NUMERIC(12,4) budget before enrollment commits. Exact maximum total99999999.9999 points is accepted; any larger total, nonpositive weight or invalid/empty total returns409 CONTENT_NOT_AVAILABLE without identity/session/cache/audit effects. Submission defensively validates its pinned budget before score writes. This is an aggregate budget, not a change to exact-set weighting or score visibility. Publication must reuse `assessment::domain::validate_score_budget(weights: impl IntoIterator<Item=i64>) -> AppResult<i64>` with weights in exact scale4 units and MAX_BANK_SCORE_UNITS999999999999, before referencing a new bank from a path.

Lesson completion timestamps are evaluated at write time after locking and never before the recorded opening; repeat completion preserves the original timestamp even if the transaction began before another opening transaction committed.

## Task3 publication/fixture integration

No implicit production seed/import exists. The explicit synthetic fixture is `tests/fixtures/student.rs::content`: two published lessons with one required section each; paired/quiz banks each containing one single and one multiple required question, three options each, exact_set_v1 and max_points1; bank versions plus ordered path version. Later frontend fixtures should drive this actual schema/API, not Node payload aliases.

CMS publications must insert complete immutable children **before** inserting referencing manifests: question-version -> options -> bank-items; lesson-version -> sections -> path-items; bank-version/items -> path-version; path-items -> enrollment; snapshot-questions -> attempt. Snapshot questions must be inserted in the same transaction that created the snapshot (migration5 seals both banks at enrollment commit, including the not-yet-open quiz). All publication inserts belong in the same transaction, so incomplete containers are not visible. Draft edits create new immutable versions instead of UPDATE/DELETE. Task3 must validate all question cardinality/key/required/rule/order and lesson/path completeness before publication. Keep school archive locking unchanged. Reporting must read attempt snapshots, not draft/latest CMS text.

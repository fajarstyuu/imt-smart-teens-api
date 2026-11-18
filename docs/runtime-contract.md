# Implemented Rust v1 runtime contract

## Trusted reverse proxy boundary

`TRUSTED_PROXY_IPS` is optional and defaults to no trusted peers. It accepts only comma-separated exact IPv4/IPv6 addresses, not wildcards/CIDRs. Only a listed TCP peer may supply one valid `X-Real-IP`; missing, malformed or duplicate values fall back to the socket peer. `Forwarded` and `X-Forwarded-For` never select rate-limit identity. The public edge must overwrite user-supplied IP headers, and the private frontend must forward that sanitized value. Never expose the backend directly or trust broad public ranges. Existing per-IP10login/300API requests per60seconds and health/preflight exclusions remain unchanged.

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
  opened_at: timestamp|null, reading_completed_at: timestamp|null,
  completed_at: timestamp|null  // lesson quiz mastered, not just read
}
Progress = {
  enrollment_id, path_version_id,
  current_stage: pre_test|lessons|calculator|post_test|completed,
  stages: [{id: same stage enum, status: locked|in_progress|completed,
            locked_reason: "LEARNING_PREREQUISITE_NOT_MET"|null}],
  lessons: [LessonMetadata + {status: locked|not_started|in_progress|completed}],
  active_attempt_ids: {pre_test?: uuid, post_test?: uuid},
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


## Admin content editor runtime (Task3)

All following routes are implemented Rust/PostgreSQL routes, under `/api/v1/admin`. They require a current active admin session. All admins have global school/content/report access; legacy booleans and grant rows no longer restrict that role. Mutation CSRF/Origin/session binding, cancellation/deadlines, no-store and request-ID/error envelopes are the same as the student and school APIs. No public configuration/bootstrap endpoint exists.

### Editor DTOs

```text
Version = {id:uuid, version:positive integer, published_at:timestamp}
SectionDraft = {id:uuid, title:string, body:string, required:boolean}
LessonDraft = {title:string, description:string, sections:SectionDraft[]}
LessonEditor = {
  id, state:draft|published|archived, revision:positive integer,
  position:positive integer|null, draft:LessonDraft, versions:Version[]
}
OptionDraft = {id:uuid, text:string}
QuestionDraft = {
  text:string, type:single|multiple, options:OptionDraft[],
  correct_option_ids:uuid[], required:true
}
QuestionEditor = {
  id, bank_id, state:draft|published|archived, revision:positive integer,
  position:positive integer, draft:QuestionDraft, versions:Version[]
}
BankEditor = {
  id, kind:paired|quiz, state:draft|published|archived,
  revision:positive integer, versions:Version[]
}
PathManifest = {
  id, version:positive integer, paired_bank_version_id, quiz_bank_version_id,
  lessons:[{lesson_id,lesson_version_id,position}]
}
PathEditor = {
  id, revision:positive integer, lesson_ids:uuid[], paired_bank_id, quiz_bank_id,
  versions:Version[], published:PathManifest|null
}
```

`versions` is newest-first immutable history metadata; `Version.id` is the immutable version UUID, **not** a root/editor ID or concurrency revision. The first entry is the newest publication, including when the root has a dirty draft. Archived lesson `position` is null because it no longer belongs to the configured draft order. The current question draft retains its last positive position after archive. `published` is the newest path manifest, not a draft preview. Histories remain readable after archive. Draft JSONB uses infrastructure schema1 and is never exposed as a raw database record.

### Complete route/payload table

Inputs below are exact JSON field names. All successes are `{data:<payload>}` except list envelopes also include top-level `meta:{limit,offset,total}`. Lists use `limit=25` by default (1–100), `offset=0` (nonnegative). Lesson/question `status` may be `draft|published|archived`; omission includes all states. Lessons sort `(current draft position NULLS LAST,id)`; questions sort `(position,id)`.

| Method/path suffix | Input/query | Success |
| --- | --- | --- |
| GET `/lessons` | `status?,limit?,offset?` | 200 `LessonEditor[]` + meta |
| GET `/lessons/{lesson_id}` | none | 200 `LessonEditor` |
| POST `/lessons` | `{title,description,sections:[{id?,title,body,required}],position}` | 201 `LessonEditor`, revision1 |
| PUT `/lessons/{lesson_id}` | create fields plus required `expected_revision` | 200 `LessonEditor` |
| POST `/lessons/{lesson_id}/publication` | `{expected_revision}` | 200 `LessonEditor`, fresh version first in history |
| POST `/lessons/{lesson_id}/archive` | `{expected_revision}` | 200 `LessonEditor` |
| GET `/learning-path` | none | 200 `PathEditor` |
| PUT `/learning-path/order` | `{lesson_ids:[uuid],expected_revision}` | 200 `PathEditor` |
| POST `/learning-path/publication` | `{expected_revision}` | 200 `PathEditor`, new `published` manifest |
| GET `/question-banks` | no query | 200 `BankEditor[]`, paired then quiz; fixed meta `{limit:2,offset:0,total:2}` |
| GET `/question-banks/{bank_id}/questions` | `status?,limit?,offset?` | 200 `QuestionEditor[]` + meta |
| GET `/question-banks/{bank_id}/questions/{question_id}` | none | 200 `QuestionEditor` |
| POST `/question-banks/{bank_id}/questions` | `{text,type,options:[{id?,text}],correct_option_ids:[uuid],position,required:true}` | 201 `QuestionEditor`, revision1 |
| PUT `/question-banks/{bank_id}/questions/{question_id}` | create fields plus required `expected_revision` | 200 `QuestionEditor` |
| POST `/question-banks/{bank_id}/questions/{question_id}/archive` | `{expected_revision}` | 200 `QuestionEditor` |
| POST `/question-banks/{bank_id}/publication` | `{expected_revision}` | 200 `BankEditor`, fresh version first in history |

Unknown JSON command fields are422; create rejects an `expected_revision` field, while update requires it (`REVISION_REQUIRED`). A valid JSON command with unsupported type/status, negative revision, invalid pagination or invalid content is422. Generic malformed path/query extraction is400; missing/invalid admin session401; student/non-admin role or CSRF/Origin denial403. Unknown lesson/bank/question, including a question under another bank, is404. Revision mismatch is409 `CONTENT_REVISION_CONFLICT`; archived mutations409 `CONTENT_ARCHIVED`. Archive is not a delete or restore, and does not erase versions. Repeating an old revision conflicts rather than creating duplicate publication rows.

### Validation, identity and revision rules

- All editorial text is **plain text**, not HTML or Markdown rendering instructions. The backend rejects `<`, `>`, inappropriate control characters and overlong fields. Frontend must render with escaped text nodes; never `innerHTML` or equivalent, including on historical data. Titles must be nonblank, up to200 Unicode characters; descriptions up to10000 (empty allowed); section bodies up to10000. Sections have distinct draft UUIDs and a maximum100. Draft lessons may have no sections or blank section bodies, but publication requires at least one section and nonblank bodies/titles. Section `required` remains a boolean, allowing optional sections.
- Question text is nonblank up to10000 characters; 2–100 nonblank options, each up to2000. Option IDs and trimmed option texts must be distinct. Keys must be a distinct nonempty subset of those IDs; single-choice requires exactly one correct option, multiple-choice accepts one or more. Initial questions require `required:true`, equal max_points1 and `exact_set_v1`. These fields/rules are not client-editable score inputs.
- Client should allocate option UUIDs **before** filling `correct_option_ids`. Missing IDs can be generated for unreferenced options, but the server never guesses a key from text, index or a missing ID. Unknown keys are422. Reuse section/option draft IDs on an ordinary edit. Every publication generates new immutable option/section row IDs; `section_key` preserves the section draft identity. Old published IDs, private keys and student snapshots are not overwritten or transplanted.
- Lesson create appends only: `position` must be the next free draft-path position, otherwise409 `USE_PATH_ORDER`. Lesson update must carry its **current** position; a requested move returns that same actionable conflict. Reorder only with PUT `/learning-path/order`, supplying every nonarchived member exactly once. Duplicate/missing/foreign IDs are422 `INVALID_PATH_ORDER`.
- Lesson create/archive atomically maintains path membership and advances the path revision. Lesson edit and lesson publication advance only the lesson revision. Order and path publication advance the path revision. All operations taking both locks acquire path before lesson; path publication acquires path, bank roots sorted by UUID, then lesson roots sorted by UUID. An editor position read before a concurrent reorder cannot overwrite that newer order: reload and explicitly reorder.
- Every question create/update/archive advances and dirties its bank revision. Bank publication compares the bank revision, validates every active persisted draft and unique positive positions plus the shared NUMERIC(12,4) aggregate budget, constructs immutable versions/options/items, advances every included question revision and the bank revision, then audits/commits. Bank root is locked before question roots. Frontend must refresh bank and question revisions after publication. Dirty question positions can be edited explicitly; duplicate positions cannot be published.
- Path publication uses the configured **explicit order**, newest existing published version of each active lesson, and newest existing published paired/quiz banks. It does not publish drafts. Dirty edits therefore do not affect an old publication. Referenced immutable lesson/question/key/rule/position/bank ownership and exact aggregate budget are validated again. Empty path422 `EMPTY_LEARNING_PATH`; empty bank publication422 `EMPTY_QUESTION_BANK`; missing/incomplete/over-budget published bank references409 `CONTENT_NOT_AVAILABLE`. Invalid published content is422. Archived question roots must be excluded via a newer valid bank publication before a new path can reference that bank. Archived lessons are removed from future draft membership.
- `expected_revision` on a path covers membership/order and previous path publications, **not** a chosen bank/lesson version token: the inventory command has no arbitrary version override. Refresh editor metadata to preview the latest published references before deliberately publishing. Existing enrollments stay on their old path, lesson, question/option/key snapshots regardless of later drafts, publications, archives or school labels.
- All mutation, root revision, immutable construction and redacted audit effects share one database transaction. No answer keys or text are duplicated into audit metadata. Publication completes children before referencing containers; migrations1–5 remain unchanged.

### Frontend version-selection workflow

Use the bank list to resolve logical paired/quiz IDs; request bounded question pages and question detail on demand. Display its current draft/revision separately from its `versions` history. A report selection stores **both** the logical question ID (optional consistency filter) and the selected `versions[n].id` as `question_version_id`; show version number/published_at so an older cohort's answers are not labeled with the latest draft. Refreshing editor content must not silently change an already selected report version. Changing version resets report pagination. Do not send only `question_id`, aggregate revisions together, or interpret a stale `expected_revision` as an immutable version. Never use admin editor objects for student rendering/bundles: they contain private draft keys.

## Admin historical reporting runtime (Task3)

Reports require a current active admin session with global access across existing, new and archived schools. Report SQL verifies the active admin actor before count/order/pagination and detail selection; legacy school grants are not consulted. Reports use read-only repeatable-read transactions for coherent metadata/rows. A school filter selects data rather than authorization. No matches gives200 empty lists with total0; nonexistent or mismatched student/enrollment/attempt details give404. Admin deactivation or session revocation is effective on subsequent requests. There is no bulk every-student-answer endpoint.

All reports use bounded top-level lists: default limit25, maximum100, offset0. Unknown, duplicate, empty, malformed or invalid filters are422 (`INVALID_REPORT_FILTER` or the more specific code below), including unknown values of stage/type and invalid UUID query values. Path UUID syntax errors remain400. Dates require exact `YYYY-MM-DD` calendar values; `form_date_from` and `form_date_to` are inclusive. Submission bounds require RFC3339 **UTC** (`Z` or `+00:00`, timezone offsets other than zero rejected) and are `[submitted_from,submitted_to)`. Inverted ranges are422 `INVALID_REPORT_RANGE`; equal submission bounds give an empty interval. A submitted range filters actual submitted attempts, excluding unstarted/in-progress ones. With `assessment_type` the same attempt must satisfy both type and time bounds.

| Method/path suffix | Accepted query fields | Success |
| --- | --- | --- |
| GET `/students` | `school_id,form_date_from,form_date_to,submitted_from,submitted_to,stage,assessment_type,limit,offset` | 200 `StudentSummary[]` + meta |
| GET `/students/{student_id}/results` | `enrollment_id` | 200 `StudentResults` |
| GET `/attempts/{attempt_id}/answers` | `limit,offset` | 200 `QuestionAnswer[]` + meta |
| GET `/reports/question-answers` | required `question_version_id`; optional `question_id,bank_id,assessment_type,school_id,submitted_from,submitted_to,limit,offset` | 200 `QuestionAnswer[]` + meta |

Student list rows are one **visible enrollment summary**, not a flattened answer export or deduplicated-human directory. Order is `(student created_at DESC,student_id DESC,enrollment created_at DESC,enrollment_id DESC)`. An `assessment_type`-only filter requires an actual attempt of that type; it does not include a not-yet-started type. Stage is the durable journey projection `pre_test|lessons|quiz|calculator|post_test|completed`, matching the student's current-stage policy. No independent mutable stage flag is stored.

Results select among all enrollments of the requested student across schools. Omit `enrollment_id` only when exactly one is visible; zero404; more than one409 `ENROLLMENT_SELECTION_REQUIRED` (select deliberately). An enrollment of another student gives404; archived schools retain their historical enrollments. Only started attempts have entries in `attempts`; missing quiz/post attempts are absent rather than zero-score placeholders. Lessons include all pinned lesson metadata and nullable opened/completed timestamps, not lesson bodies. Measurements are this one enrollment's stored history, newest `(created_at,id)` first, using the existing exact measurement DTO. Results do not paginate these per-enrollment histories; use bounded answer endpoints for question-level inspection.

```text
StudentSummary = {
  student_id, name, enrollment_id, school_id, school_name_at_enrollment,
  form_date, path_version_id, created_at, completed_at:timestamp|null,
  current_stage, latest_submitted_at:timestamp|null
}
AttemptSummary = {
  id, snapshot_id, assessment_type, attempt_number, status:in_progress|submitted,
  created_at, submitted_at:timestamp|null, earned_points:number|null,
  max_points:number|null, score_percent:integer|null
}
StudentResults = {
  student:Student, enrollment:Enrollment, current_stage,
  latest_submitted_at:timestamp|null, attempts:AttemptSummary[],
  lessons:LessonMetadata[], measurements:Measurement[]
}
QuestionAnswer = {
  student_id, student_name, enrollment_id, school_id, school_name_at_enrollment,
  bank_id, bank_version_id, snapshot_id, snapshot_question_id,
  question_id, question_version_id, position, text, type:single|multiple,
  assessment_type, attempt_id:uuid|null, attempt_number:integer|null,
  attempt_status:in_progress|submitted|null, answer_revision:integer|null,
  saved_at:timestamp|null, submitted_at:timestamp|null,
  options:[{id,text}], selected_option_ids:uuid[], selected_options:[{id,text}],
  answer_state:not_started|unanswered|answered|incorrect|correct,
  correctness:boolean|null, earned_points:number|null, max_points:number
}
```

`StudentSummary.created_at` is enrollment creation time (the internal primary list sort additionally uses student creation time). Question/option/selected text and all snapshot/version IDs come from immutable historical references, **never** latest editor drafts. Point fields are exact scale4 JSON numeric tokens (e.g. `1.0000`, `0.0000`), constructed from checked scaled integers without floating point; preserve raw decimal tokens if frontend exports exact decimals. Percent follows the existing integer half-up scoring policy. Admin historical grading is visible with school scope even while the student pre/post reveal policy hides their score. No raw private answer-key list is returned in reports.

For one attempt the answer page is pinned snapshot order `(position,snapshot_question_id)` (the shared stable query additionally orders fixed attempt/enrollment metadata). Across students the stable order is `(submitted_at DESC NULLS LAST,attempt_id DESC NULLS LAST,enrollment_id DESC,assessment_type,position,snapshot_question_id)`. The selected version determines membership: paired snapshot questions produce separate pre/post rows per enrollment, even when post is not yet started; quiz questions produce quiz rows only. If the attempt does not exist, attempt ID/number/status and submission time are null. An unknown version alone yields an empty scoped page; optional `bank_id`/`question_id` must agree with the selected immutable version or422 `INVALID_QUESTION_SELECTION`. Missing version, including question-ID-only requests, is422 `QUESTION_VERSION_REQUIRED` with a version-selection message.

Answer state is intentionally not a boolean:

| Stored facts | answer_state | correctness / earned |
| --- | --- | --- |
| no answer row, including a not-yet-started attempt | `not_started` | null / null |
| saved row with zero selected options | `unanswered` | null while pending; historical persisted grading if submitted |
| nonempty selection, unsubmitted attempt | `answered` | null / null (not incorrectly marked wrong) |
| submitted nonempty exact-key match | `correct` | true / stored exact points |
| submitted nonempty mismatch | `incorrect` | false / stored exact zero |

`answer_revision` is null for no row (not a fabricated autosave revision); clearing retains a positive revision and saved timestamp. Empty selections are not evidence that a question/attempt was never started. Counts include authorized pinned rows before answer-state projection, so clearing/saving does not change report membership. There is no lookup by a student's name or school label.

## Explicit development commands and application entry points

From this backend worktree, with the existing Rust toolchain:

```sh
export PATH=/home/ubuntu/.cargo/bin:$PATH
export CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
# Load an operator-owned private DEVELOPMENT settings file; never print its contents.
set -a; . /path/to/private/development.env; set +a
cargo run --locked -- migrate
APP_ENV=development cargo run --locked -- init-content-config
HTTP_HOST=127.0.0.1 HTTP_PORT=4180 APP_ENV=development cargo run --locked -- serve
```

The placeholder file path is not a committed file. It must supply the intended development `DATABASE_URL`, separate DDL `MIGRATION_DATABASE_URL`, private `CSRF_SIGNING_KEY` (at least32 bytes), and allowlisted frontend origin (local frontend port5173). Use insecure session cookies only on loopback development HTTP; production still requires secure settings. No migrations run automatically during `serve` or config initialization. Do not point test/init commands at an application/production database by accident. Task3 verification only used SQLx-created isolated databases; the existing application database was not migrated or modified.

`init-content-config` is an explicit private CLI command taking **no arguments**. Its application entry point is `learning::admin::application::LessonCms::initialize_development(environment: &str) -> AppResult<()>`, backed by `LessonStore::initialize_empty`. It only accepts `development`, rejects production/test/unknown modes, serializes concurrent initializers with an advisory transaction lock, inserts a missing empty paired-assessment root and one empty configured path draft with nullable legacy global-quiz bank reference, and preserves all existing IDs/revisions/drafts/publications. Multiple existing configured paths fail409 rather than guessing one. It creates **no** schools, questions, lessons, publications, students, sessions, sample credentials or answer content. A missing configured path/bank pair is409 `CONTENT_CONFIGURATION_REQUIRED` from the relevant editor facade; GET never initializes or publishes. Content must then be authored/published deliberately through authenticated admin endpoints.

The existing private `provision-admin` command remains separate. Supply ADMIN_EMAIL/ADMIN_NAME and the password privately via the existing environment/pipe convention, never command arguments; set ADMIN_CAN_MANAGE_CONTENT and/or ADMIN_CAN_MANAGE_SCHOOLS explicitly as needed. Provisioning creates no reporting grants. Reporting grants require a separate authorized operator assignment to existing school IDs; there is no public permission/grant endpoint. No default admin account or teaching seed command was added.

Bootstrap injects `AppState.lesson_cms: LessonCms`, `bank_cms: BankCms`, and `reports: Reports`. Core-owned ports are `learning::admin::application::LessonStore`, `assessment::admin::application::BankStore`, and `reporting::application::ReportReader`. Editorial core types/commands and report results contain no Axum, SQLx, environment access, JSON Value/Number or HTTP DTO formatting. Common editorial metadata/pagination/text rules are in `shared::editing`; content actor authorization belongs to identity application; neutral persistence/audit helpers are infrastructure in `platform::editor`.

Path publication is an explicit core workflow, not a foreign-context infrastructure validation API. `LessonStore::begin_path_publication` returns the learning-owned `PathPublicationTransaction`, extending assessment's application-owned `PublishedBankReader`. `LessonCms::publish_path` authorizes, then `publish_path_in_transaction` coordinates path lock/revision/order validation, sorted bank locks and typed `PublishedBank::admit`, sorted lesson locks and typed published lesson admission, admitted `PathPublication` persistence, and consuming commit. Assessment owns `BankKind`/`PublishedBank`/`PublishedQuestion` facts and the policy for root/version ownership, kind/archive/rule/order, question options/keys/cardinality/required and shared exact scale4 budget. Learning owns lesson completeness/archive and path membership decisions. Adapters return availability/state facts rather than deciding admission by SQL filtering. The concrete `PgPathPublication` composes assessment's transaction-scoped `PgPublishedBankReader` over the **same borrowed transaction connection**; no second pooled bank transaction, driver argument in an application signature, or manifest write before all references pass admission. SQL/locks/mapping/persistence/audit remain in adapters. Error/status behavior and lock order above are unchanged.

All Task3 editor/report responses are compiler-checked `Serialize` HTTP DTOs, independent of core types and infrastructure JSONB: editor DTO modules contain concrete drafts/sections/options/private keys/version histories/manifests; reporting DTOs contain concrete summaries/results/attempts/question answers/selected options. Shared `Data<T>`, `PageDto<T>`, `PageMeta` and `VersionDto` define the existing envelopes/history metadata. Handlers return `Json<Data<ConcreteDto>>`, `Json<PageDto<ConcreteDto>>` or the typed201 tuple, not `Json<Value>`. Nested fields are typed rather than untyped JSON containers. Reporting reuses the existing safe student/enrollment/lesson/measurement transport DTOs; admin draft-key types are not reused in reports or students. Exact scale4 point `Number` tokens are produced only in reporting's HTTP mapper. All nullable fields serialize explicit null (no `skip_serializing_if`); field names, numeric/string types, states, ordering, presence/privacy and statuses documented above are unchanged.

To reproduce the synthetic HTTP publication/enrollment/completion/historical-reporting proof without adding content to a running application:

```sh
# .env.test is the existing ignored private SQLx test configuration; do not display it.
set -a; . ./.env.test; set +a
cargo test --locked --test admin_content --test cms_workflow --test cms_concurrency --test reporting --test cms_response_contract --test publication_policy -- --test-threads=1
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked -- --test-threads=1
cargo build --locked
python3 -m unittest discover -s tests/infra -p 'test_*.py' -v
```

These tests author synthetic content through the real router/PostgreSQL adapters; test-only fixtures are not bootstrap content or an alternative production backend. `cms_workflow` runs CMS publication -> enrollment -> later CMS publications/archive -> full original student completion -> authorized historical reporting, including school rename/archive and exact snapshot/label preservation. There are no501 routes or fixture JSON handlers in the runtime.

## Lesson-linked quiz additions (worker checkpoint)

The lesson editor's `draft` now adds `quiz: LessonQuizDraft|null`:

```text
LessonQuizDraft = {
  text: string,
  options: [{id: uuid, text: string}, {id: uuid, text: string}],
  correct_option_id: uuid|null
}
```

This is an admin-only DTO. It is optional while drafting; provided option IDs must be distinct/non-nil, and any present key must identify one of them. Publishing requires nonblank question and both option texts and exactly one key. Every newly published lesson version stores `quiz_payload` JSONB in the same transaction as its sections and audit; the forward migration leaves older version rows nullable. This change has real PostgreSQL coverage through existing CMS publication tests and unit/domain checks in `tests/lesson_quiz.rs`.

Private operational addition: `imt-smart-teens-backend init-empty-content` runs the serialized structural initializer in any configured environment, including production. It creates only a missing empty paired assessment root and one path draft whose legacy quiz-bank reference is null. It is not called on serve; `init-content-config` keeps its development-only guard. The synthetic SQLx test uses an APP_ENV=production subprocess, confirms repeat idempotence, no lesson/version creation, and unchanged admin password hash. This command is not to be run against live production in this implementation.

### Frozen lesson-linked student contract (2026-11-18)

The global `quiz` assessment-attempt stage is retired for new progress. Legacy global quiz attempts/banks remain in storage for historical compatibility, but `POST /api/v1/student/attempts` with `assessment_type:"quiz"` is rejected by the domain prerequisite. New enrollments pin exactly one shared paired pre/post snapshot. `pre_test`/`post_test` types, answer cardinality and paired snapshot identity remain unchanged. Calculator and post-test are gated on mastery of every pinned lesson quiz; POST attempts, direct measurement calls and old quiz endpoints cannot bypass this server-side gate.

`GET /api/v1/student/lessons/{lesson_id}/quiz` returns `{lesson_id,lesson_version_id,question,options:[{id,text},{id,text}],first_answer_option_id,attempt_count,revision,mastered}`. It exposes no answer key. It requires submitted pre-test, prior lesson mastery and the target lesson's `reading_completed_at`; lesson reads/required-section completion are separate from quiz mastery. A lesson becomes completed/unlocks the next lesson only when its immutable version's quiz is answered correctly.

`POST /api/v1/student/lessons/{lesson_id}/quiz/answers` requires the normal student cookie, cookie-bound CSRF token, allowlisted Origin, `Idempotency-Key`, and `{option_id,expected_revision}`. One selected option is accepted per request. It returns `{lesson_id,lesson_version_id,selected_option_id,correct,feedback,first_answer_option_id,attempt_count,revision,mastered,progress}`. Wrong answers are committed and count; their `feedback` is exactly `jawablah dengan benar untuk melanjutkan materi selanjutnya`. Retrying is unlimited. The first option is immutable. Each accepted selection advances revision and gets a history row; repeated same-key/same-payload requests replay without incrementing. Stale revisions, foreign options, and attempts after mastery are rejected. Attempt ownership is `(enrollment_id,lesson_version_id)`, so future publications cannot retarget an existing enrollment.

`LessonMetadata` distinguishes `reading_completed_at` from quiz-mastered `completed_at`. Lesson quiz history is exposed only in `GET /api/v1/admin/students/{student}/results` as `lesson_quizzes:[{lesson_id,lesson_version_id,lesson_title,question,options,first_answer,attempt_count,status}]`, where status is `not_started|retry_required|mastered`; no correct key is included. The main student list has no new quiz columns and no per-lesson history endpoint is added.

CMS `GET /api/v1/admin/question-banks` now lists only the paired pre/post bank. Reviewed path publication validates/publishes that paired bank plus complete lesson quizzes; a shared global quiz bank is neither listed nor required. `PathEditor.quiz_bank_id` and historical manifest `quiz_bank_version_id` are nullable legacy fields. `imt-smart-teens-backend init-empty-content` is the explicit production-safe initialization command; this implementation did not run it live.

# API inventory and frontend migration contract

**Status: target design, not implemented endpoints.** The PostgreSQL foundation does not implement these APIs. Student identity is not authentication proof; a matching name/school never recovers someone else's records.

## Source and owner decisions

- Rules: `fajarstyuu/imt-smart-teen`, commit `8df5bafbe8912e37d2e0ba898732bc9f5d9159b7`: `AGENTS.md`, `docs/rules.md`, `docs/backend.md`, `docs/frontend.md`, `docs/delegation.md`, `docs/handoff.md` and relevant `docs/handoffs/2026-11-14-imt-web-routing-brand.md`.
- Frontend: `fajarstyuu/imt-smart-teens-web`, commit `6344f0b7304d9cfcd5b642034ccf33b2c3393585`; actual `src/services/api.ts`, `src/services/contracts.ts`, `src/services/http.ts`, `src/domain/types.ts`, `src/hooks/useTeacherData.ts`, and development API in `server/development.mjs` / `server/domain.mjs`.
- Initial backend: `fajarstyuu/imt-smart-teens-api`, commit `40bb0f5b61cfd7b4c0a69d4dd141bc5818a63389`; no Rust implementation at baseline.
- Owner requires **admin**, not teacher, and school CMS add/edit/delete. Target roles/routes/cookies/tables use admin; legacy teacher names in the migration table are observed source, not target endpoints.
- School deletion means archive, not physical deletion. Students select an active catalog school ID; no free-text creation at login. Catalog permissions do not imply unrestricted school-report access.
- CMS covers shared paired pre/post bank, separate quiz bank, versioned lesson content/order/publication and scoped reporting. Full UI integration is a future milestone.
- Session TTLs, field limits, CSRF mechanics and scoring policy below are proposed implementation decisions, not already deployed guarantees.


### Common transport decisions

- IDs: UUID strings. Timestamps: RFC3339 UTC. `form_date`: real calendar date `YYYY-MM-DD`, not converted to timestamp. External assessment types: `pre_test|quiz|post_test`; frontend adapter maps `pre|quiz|post`.
- Success: `{ "data": ... }`; paginated lists add `meta:{limit,offset,total}`. `limit` default 25, max 100; offset default 0. Stable ordering `(created_at DESC,id DESC)` for students, `(submitted_at DESC,id DESC)` for submitted attempts; content ordered `(position,id)`.
- Errors: `{error:{code,message,details?},request_id}` with Indonesian message; `400` malformed transport, `401` absent/expired session, `403` wrong role/school scope or invalid CSRF, `404` absent/not-owned resource, `409` prerequisite/revision/idempotency/state conflict, `422` valid JSON with invalid values/unanswered required questions, `429` rate limit, `503` unavailable DB. Sensitive endpoints `Cache-Control:no-store`. Bodyless `204` must not be JSON-parsed by the eventual frontend adapter.
- Cookie names `imt_student_session`, `imt_admin_session`; opaque random tokens, store only SHA-256 token hashes server-side. `HttpOnly`, path `/api/v1`, host-only; `Secure` in production; `SameSite=Lax` for proposed same-origin deployment. Independent student/admin sessions can coexist.
- `GET /api/v1/csrf` creates/refreshes a short-lived browser CSRF context and returns `{token}`; validate its cookie-bound token AND allowlisted Origin for login and all mutations, using `X-CSRF-Token`. Bind/rotate CSRF context on login. No CORS `*` with credentials. Defaults proposed: idle 2 hours, absolute 7 days student / 12 hours admin; confirm deployment settings before implementation.
- `Idempotency-Key` required on student enrollment, attempt creation, submission and measurement POSTs. Scope: principal+operation, or pre-login CSRF context+operation for enrollment. TTL 24h; hash canonical validated payload; same key+payload returns original result, different payload yields `409 IDEMPOTENCY_CONFLICT`. Store reservation/response in the same transaction as business effects. Already-submitted attempt also returns its recorded result when requested with a new key; never scores twice.
- `expected_revision` is a nonnegative answer revision, initially 0 for an absent answer. Question version identifies immutable content and does NOT substitute for this revision. Concurrent stale save returns `409 ANSWER_REVISION_CONFLICT` with latest safe answer/revision.
- Proposed string limits: name 1–200 trimmed characters, school label 1–300, lesson title 1–200, question text 1–10000, option text 1–2000. No two-word-name requirement. These limits are implementation proposals, not existing validation guarantees.

### Public and session endpoints

- `GET /health/live` → `200 {data:{status:"live"}}`, process-only.
- `GET /health/ready` → `200 {data:{status:"ready"}}`, short DB check; `503` during dependency failure/draining.
- `GET /api/v1/config` → public brand/program/environment only, no credentials.
- `GET /api/v1/csrf` → browser CSRF token, no personal data.
- `POST /api/v1/student/sessions` body `{name,school_id,form_date}` → `201 {data:{student,enrollment}}` plus student cookie; creates a new identity/enrollment, pins path version and paired snapshot, creates pre-test attempt atomically. Idempotent retry returns original status/body. If published learning path/paired bank/quiz unavailable, `409 CONTENT_NOT_AVAILABLE`, no partial student/session.
- `GET /api/v1/student/me` → current identity/enrollment IDs and dates; never match a record by submitted name/school.
- `DELETE /api/v1/student/sessions/current` → revoke that cookie/session, `204`; repeat logout still `204` with valid CSRF context.
- `POST /api/v1/admin/sessions` body `{email,password}` → `201 {data:{admin:{id,name,email}}}` plus admin cookie; Argon2id credentials, generic `401` for invalid credentials.
- `GET /api/v1/admin/me` → current admin account and safe display scope.
- `DELETE /api/v1/admin/sessions/current` → revoke admin session, `204`.

### Student learning endpoints

All derive student/enrollment from the cookie, never a client body `student_id`.

- `GET /api/v1/student/progress` → `{enrollment_id,path_version_id,current_stage,stages:[{id,status,locked_reason}],lessons:[{lesson_id,lesson_version_id,title,position,status,opened_at,completed_at}],active_attempt_ids,latest_measurement_id}`. Metadata only for locked content. Derive locks from durable records; do not store independent mutable percentage/stage flags.
- `GET /api/v1/student/lessons/{lesson_id}` → allowed pinned version, sections and own visited/completed state; `409 LEARNING_PREREQUISITE_NOT_MET` when locked.
- `POST /api/v1/student/lessons/{lesson_id}/opened` body `{section_id?:uuid}` → `200 {data:{lesson_id,opened_at,visited_section_ids}}`; validate section against pinned version. Repeated visits do not duplicate rows; omitted section records opening only.
- `POST /api/v1/student/lessons/{lesson_id}/completion` body `{}` → `200 {data:{lesson_id,completed_at,progress}}`; all required sections visited, prerequisite enforced under enrollment lock; repeats preserve original timestamp.
- `POST /api/v1/student/attempts` body `{assessment_type}` → `201` newly created or `200` existing; `{data:{attempt_id,assessment_type,attempt_number,status,snapshot_id}}`. Existing pre-test returned after enrollment. Quiz after all lessons; post-test after committed measurement; exactly one final submission per type initially.
- `GET /api/v1/student/attempts/{attempt_id}` → `{id,assessment_type,attempt_number,status,snapshot_id,questions:[{id,question_id,question_version_id,position,text,type,options:[{id,text}]}],answers:[{snapshot_question_id,option_ids,revision,saved_at}],submitted_at,score}`. Score nullable/hidden according to reveal policy; no answer keys/explanations before permitted reveal. `id` on a question is its snapshot-question ID; save path uses this ID to disambiguate snapshots.
- `PUT /api/v1/student/attempts/{attempt_id}/answers/{snapshot_question_id}` body `{option_ids:[uuid],expected_revision:integer}` → `200` committed `{snapshot_question_id,option_ids,revision,saved_at}`. Empty array clears an answer while open; single-choice at most one; reject duplicate/foreign IDs. Post-submission saves `409 ATTEMPT_SUBMITTED`.
- `POST /api/v1/student/attempts/{attempt_id}/submission` body `{}` → `200 {data:{attempt_id,status:"submitted",submitted_at,score,progress}}`; lock attempt, reject missing required answers with `422`, score from private snapshot, transition atomically. Quiz score immediately available; pre/post scores remain `null` until post-test completed. Exact-match multiple-choice, equal weight initially; percent rounded nearest integer, stored raw earned/max too. Pre/post use identical question/option/key/order/rule snapshot.
- `POST /api/v1/student/measurements` body `{weight_kg:number,height_cm:number}` → `201 {data:{id,weight_kg,height_cm,bmi,bmi_display,calculation_rule_version,created_at,progress}}`; positive finite supported decimals, recompute on server; `bmi_display` one decimal, no clinical categories. New measurement only after quiz and before post-test submission (preserves current UI behavior); repeated idempotent retry returns same measurement.
- `GET /api/v1/student/measurements` → own history paginated, newest first. Needed for refresh/history; no cross-student records.
- `GET /api/v1/student/summary` → own scores, stage status and latest measurement only after post-test; `409` beforehand. Does not expose other students or live CMS keys.

### Admin school CMS endpoints

School management is an additional owner-requested CMS capability. Treat deletion as soft deletion (archive) for all schools; do not add a destructive hard-delete route. This preserves enrollment/reporting history and foreign keys. A separate restore feature is out of scope.

- Authorization: admin cookie, `can_manage_schools` permission, valid CSRF and allowed Origin for mutations. Catalog management does not confer access to every school's student results; reporting still requires school grants. No public creation or school creation as a side effect of student login.
- `GET /api/v1/schools?q=&limit=&offset=` → public active-school picker only, `{data:[{id,name}],meta:{limit,offset,total}}`. Default limit 25, max 100, trimmed case-insensitive substring search, stable `(normalized_name,id)` ordering. Do not expose student counts, admin grants or archived entries.
- `GET /api/v1/admin/schools?q=&status=active|archived|all&limit=&offset=` → authorized catalog list, default active, same pagination/order; rows `{id,name,status,revision,created_at,updated_at,archived_at}`.
- `GET /api/v1/admin/schools/{school_id}` → catalog detail with the same fields, including archived entries.
- `POST /api/v1/admin/schools` body `{name}` → `201 {data:{id,name,status:"active",revision:1,created_at,updated_at,archived_at:null}}`. Trim/collapse whitespace, Unicode NFC normalization and case folding for duplicate detection; name 1–300 characters after normalization. Reject blank/overlong names with `422 INVALID_SCHOOL_NAME`, duplicate normalized names (including archived names) with `409 SCHOOL_NAME_EXISTS`.
- `PUT /api/v1/admin/schools/{school_id}` body `{name,expected_revision}` → `200` updated catalog detail; increment revision atomically, reject stale revisions with `409 SCHOOL_REVISION_CONFLICT`. Archived-school edits return `409 SCHOOL_ARCHIVED`; absent ID `404`. Renaming must preserve IDs, grants and enrollment-time school-name snapshots.
- `DELETE /api/v1/admin/schools/{school_id}?expected_revision=N` → `200 {data:{id,status:"archived",archived_at,revision}}`; atomically set archived_at and increment revision; stale revision `409`, absent ID `404`. Repeating with the current archived revision returns the recorded archived state without another mutation. Never cascade-delete enrollments, attempts, measurements or grants.
- Student registration target now requires `{name,school_id,form_date}`, not free-text `school`. Verify the UUID resolves to an active school under the same row lock used for edit/archive; invalid UUID `400`, missing field `422`, unknown or archived school `422 SCHOOL_NOT_AVAILABLE`. Archive racing registration must serialize: a registration committed first remains valid, one after archive fails without partial records. Student refresh/results still work after archive.
- CMS UI follow-up: new `web/src/pages/AdminSchoolsPage.tsx`, registration picker in existing `web/src/pages/LoginPage.tsx`, route/navigation in `web/src/App.tsx` and existing `web/src/components/TeacherShell.tsx`, API/contracts/types in existing service files. Preserve actual legacy filenames until a separately scoped rename. Include loading/empty/error states, search/pagination, add/edit forms, delete confirmation, pending-request controls, preserved input on failure and Indonesian labels. List archived schools as read-only history. No new frontend files are implemented in the PostgreSQL-only milestone.

### Admin content endpoints

Require admin role and content-management grant; no public admin enrollment or production seed credentials. Lists paginated. `expected_revision` on editing/publish/order/archive; conflict `409 CONTENT_REVISION_CONFLICT`. Editor response includes allowed admin-only keys.

- `GET /api/v1/admin/lessons?status=draft|published|archived&limit=&offset=`; `GET /api/v1/admin/lessons/{lesson_id}` includes editable revision/history metadata.
- `POST /api/v1/admin/lessons` body `{title,description,sections:[{id?,title,body,required}],position}` → `201` draft ID/revision; server generates IDs where absent.
- `PUT /api/v1/admin/lessons/{lesson_id}` same fields plus `expected_revision` → `200` new draft revision; never mutate pinned versions.
- `POST /api/v1/admin/lessons/{lesson_id}/publication` body `{expected_revision}` → `200` new immutable version.
- `POST /api/v1/admin/lessons/{lesson_id}/archive` body `{expected_revision}` → `200` archived metadata, historical references retained.
- `PUT /api/v1/admin/learning-path/order` body `{lesson_ids:[uuid],expected_revision}` → `200` draft path revision. Require every nonarchived path member exactly once; no duplicates/unknowns.
- `GET /api/v1/admin/learning-path` → current draft/published version and ordered lesson-version references.
- `POST /api/v1/admin/learning-path/publication` body `{expected_revision}` → `200` immutable ordered manifest, referenced published paired/quiz bank versions. Requires at least one lesson and nonempty valid banks; affects future enrollments only.
- `GET /api/v1/admin/question-banks` → two logical banks, `paired` and `quiz`, statuses/revisions.
- `GET /api/v1/admin/question-banks/{bank_id}/questions?status=&limit=&offset=`; `GET /api/v1/admin/question-banks/{bank_id}/questions/{question_id}` → editor detail/history.
- `POST /api/v1/admin/question-banks/{bank_id}/questions` body `{text,type:"single"|"multiple",options:[{id?,text}],correct_option_ids,position,required:true}` → `201` draft question/revision.
- `PUT /api/v1/admin/question-banks/{bank_id}/questions/{question_id}` same fields plus `expected_revision` → `200` new draft revision. Validate at least two nonblank options, distinct IDs, nonempty key subset; exactly one key for single-choice.
- `POST /api/v1/admin/question-banks/{bank_id}/questions/{question_id}/archive` body `{expected_revision}` → `200`; no permanent delete.
- `POST /api/v1/admin/question-banks/{bank_id}/publication` body `{expected_revision}` → `200` immutable bank version referencing validated question versions. Draft save does not silently publish.

### Admin reporting endpoints

Require admin's explicit school grants; content editor grant does not imply unrestricted student-data access.

- `GET /api/v1/admin/students?school_id=&form_date_from=&form_date_to=&submitted_from=&submitted_to=&stage=&assessment_type=&limit=&offset=` → bounded summary rows, not every answer and lesson body. Submitted bounds RFC3339 UTC `[from,to)`; form-date bounds inclusive calendar dates; stage enum `pre_test|lessons|quiz|calculator|post_test|completed`.
- `GET /api/v1/admin/students/{student_id}/results?enrollment_id=` → scoped enrollment summary, attempt metadata, lesson events and measurement history. Enrollment optional only if exactly one is visible; otherwise require selection.
- `GET /api/v1/admin/attempts/{attempt_id}/answers?limit=&offset=` → historical question text, option text, selected IDs/text, earned/max points, correctness nullable, attempt number, immutable version IDs, saved/submitted timestamps. Distinguish `not_started`, `unanswered`, `incorrect`, `correct`.
- `GET /api/v1/admin/reports/question-answers?bank_id=&question_id=&question_version_id=&assessment_type=&school_id=&submitted_from=&submitted_to=&limit=&offset=` → answers across scoped students; question/version selection mandatory so results do not silently combine revisions.

### Current frontend → target mapping / explicit integration backlog

Current paths below have `/api` prefix. They remain development-only; no legacy server aliases in this milestone.

| Current frontend call | Target / integration decision |
| --- | --- |
| `GET /bootstrap` | adapter composes config, student me/progress/current attempts/allowed lesson content and admin me; anonymous `401` becomes null identity, other failures remain errors |
| `POST /students {name,school,date}` | replace free-text school with active-school picker and send `school_id`; map `date`→`form_date`; cookie, no returned bearer token |
| `POST /logout` role header | role-specific DELETE session; support bodyless 204 |
| `POST /teacher/login` / `GET /teacher/me` | admin sessions / admin me |
| `POST /attempts/{pre\|quiz\|post}` | create/get target attempt, then GET by ID; retain ID in adapter |
| `PUT /attempts/{type}/answers {questionId,version,value}` | PUT by attempt and snapshot question ID; map value→option_ids; persist separate answer revision; content version is not concurrency revision |
| `POST /attempts/{type}/submit` | POST submission, then refresh safe Student projection |
| `POST /lessons/{id}/visit {sectionId}` | POST opened `{section_id}` |
| `POST /lessons/{id}/complete` | POST completion |
| `POST /measurement {weight,height}` | POST measurements with explicit units, then refresh Student projection |
| `GET /teacher/results` returning every Student | paginated student list, load scoped detail/answers on demand; UI changes required, do not just download every page forever |
| `GET /teacher/content` | compose paginated lessons/banks/questions in editor views, no unbounded mega-endpoint |
| `PUT /teacher/lessons` upsert+published | explicit create/update and separate publication; carry revision |
| `PUT /teacher/questions/{paired\|quiz}` | resolve bank ID, explicit create/update; bank publication separate |
| `POST /teacher/archive {kind,id}` | resource-specific archive; no hard delete |
| `PUT /teacher/order {ids}` | learning-path draft order + explicit publication |

Future frontend ownership: `web/src/services/api.ts`, `web/src/services/http.ts`, `web/src/services/contracts.ts`, `web/src/domain/types.ts`, `web/src/hooks/useAssessment.ts`, `web/src/pages/TeacherResultsPage.tsx`, `web/src/pages/TeacherContentPages.tsx`, relevant tests. This list is a follow-up boundary, NOT authorization to edit now. Replace token storage with `credentials:'include'`, CSRF header handling, envelope/error mapping and revisions in that follow-up. Never copy the development admin password or return Node fixtures as production data.

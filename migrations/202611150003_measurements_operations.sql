CREATE TABLE anthropometry_measurements (
 id uuid PRIMARY KEY, enrollment_id uuid NOT NULL REFERENCES learning_enrollments ON DELETE RESTRICT,
 weight_kg numeric(12,4) NOT NULL CHECK(weight_kg>0 AND weight_kg<'Infinity'::numeric),
 height_cm numeric(12,4) NOT NULL CHECK(height_cm>0 AND height_cm<'Infinity'::numeric),
 bmi numeric(38,12) NOT NULL CHECK(bmi>0 AND bmi<'Infinity'::numeric),
 calculation_rule_version text NOT NULL CHECK(calculation_rule_version='bmi_v1'), created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE platform_idempotency (
 scope_hash bytea NOT NULL CHECK(octet_length(scope_hash)=32), operation text NOT NULL,
 key text NOT NULL CHECK(octet_length(key) BETWEEN 1 AND 128 AND key ~ '^[ -~]+$'),
 payload_hash bytea NOT NULL CHECK(octet_length(payload_hash)=32), status_code integer NOT NULL CHECK(status_code BETWEEN 200 AND 599),
 response_body jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), expires_at timestamptz NOT NULL,
 PRIMARY KEY(scope_hash,operation,key), CHECK(expires_at>created_at)
);
CREATE TABLE platform_audit_events (
 id uuid PRIMARY KEY, actor_session_id uuid REFERENCES identity_sessions ON DELETE RESTRICT,
 actor_admin_id uuid REFERENCES identity_admins ON DELETE RESTRICT, action text NOT NULL, resource_type text NOT NULL,
 resource_id uuid NOT NULL, resource_version text, request_id text NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), safe_metadata jsonb NOT NULL DEFAULT '{}'
);
CREATE INDEX enrollments_student ON learning_enrollments(student_id,created_at DESC,id DESC);
CREATE INDEX enrollments_school_date ON learning_enrollments(school_id,form_date,id);
CREATE INDEX enrollments_school_created ON learning_enrollments(school_id,created_at DESC,id DESC);
CREATE INDEX attempts_enrollment ON assessment_attempts(enrollment_id,assessment_type,status);
CREATE INDEX attempts_submitted ON assessment_attempts(assessment_type,submitted_at DESC,id DESC);
CREATE INDEX answers_question ON assessment_answers(snapshot_question_id,attempt_id);
CREATE INDEX snapshot_questions_version ON assessment_snapshot_questions(question_version_id,snapshot_id);
CREATE INDEX measurements_enrollment ON anthropometry_measurements(enrollment_id,created_at DESC,id DESC);
CREATE INDEX sessions_idle ON identity_sessions(idle_expires_at);
CREATE INDEX sessions_absolute ON identity_sessions(absolute_expires_at);
CREATE INDEX idempotency_expiry ON platform_idempotency(expires_at);
CREATE INDEX audit_created ON platform_audit_events(created_at,id);
CREATE INDEX audit_resource ON platform_audit_events(resource_type,resource_id,created_at);

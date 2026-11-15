-- Foreign-key tuples explicitly name ownership; a UUID alone is not authority.
CREATE TABLE learning_path_versions (
 id uuid PRIMARY KEY, version integer NOT NULL UNIQUE CHECK(version>0),
 paired_bank_version_id uuid NOT NULL REFERENCES assessment_bank_versions ON DELETE RESTRICT,
 quiz_bank_version_id uuid NOT NULL REFERENCES assessment_bank_versions ON DELETE RESTRICT, published_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE learning_path_items (
 path_version_id uuid NOT NULL REFERENCES learning_path_versions ON DELETE RESTRICT,
 lesson_version_id uuid NOT NULL REFERENCES learning_lesson_versions ON DELETE RESTRICT,
 position integer NOT NULL CHECK(position>0), PRIMARY KEY(path_version_id,lesson_version_id), UNIQUE(path_version_id,position)
);
CREATE TABLE learning_enrollments (
 id uuid PRIMARY KEY, student_id uuid NOT NULL REFERENCES identity_students ON DELETE RESTRICT,
 school_id uuid NOT NULL REFERENCES identity_schools ON DELETE RESTRICT, school_name_at_enrollment text NOT NULL,
 form_date date NOT NULL, path_version_id uuid NOT NULL REFERENCES learning_path_versions ON DELETE RESTRICT,
 created_at timestamptz NOT NULL DEFAULT now(), completed_at timestamptz, UNIQUE(id,student_id)
);
CREATE TABLE learning_lesson_progress (
 enrollment_id uuid NOT NULL REFERENCES learning_enrollments ON DELETE RESTRICT,
 lesson_version_id uuid NOT NULL REFERENCES learning_lesson_versions ON DELETE RESTRICT,
 opened_at timestamptz NOT NULL, completed_at timestamptz, PRIMARY KEY(enrollment_id,lesson_version_id),
 CHECK(completed_at IS NULL OR completed_at>=opened_at)
);
CREATE TABLE learning_section_visits (
 enrollment_id uuid NOT NULL, lesson_version_id uuid NOT NULL, section_id uuid NOT NULL, first_visited_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY(enrollment_id,section_id),
 FOREIGN KEY(enrollment_id,lesson_version_id) REFERENCES learning_lesson_progress ON DELETE RESTRICT,
 FOREIGN KEY(lesson_version_id,section_id) REFERENCES learning_sections(lesson_version_id,id) ON DELETE RESTRICT
);
CREATE TABLE assessment_snapshots (
 id uuid PRIMARY KEY, enrollment_id uuid NOT NULL REFERENCES learning_enrollments ON DELETE RESTRICT,
 kind text NOT NULL CHECK(kind IN ('paired','quiz')), bank_version_id uuid NOT NULL REFERENCES assessment_bank_versions ON DELETE RESTRICT,
 created_at timestamptz NOT NULL DEFAULT now(), UNIQUE(enrollment_id,kind), UNIQUE(id,enrollment_id)
);
CREATE TABLE assessment_snapshot_questions (
 id uuid PRIMARY KEY, snapshot_id uuid NOT NULL REFERENCES assessment_snapshots ON DELETE RESTRICT,
 question_version_id uuid NOT NULL REFERENCES assessment_question_versions ON DELETE RESTRICT, position integer NOT NULL CHECK(position>0),
 UNIQUE(snapshot_id,question_version_id), UNIQUE(snapshot_id,position), UNIQUE(snapshot_id,id), UNIQUE(snapshot_id,id,question_version_id)
);
CREATE TABLE assessment_attempts (
 id uuid PRIMARY KEY, enrollment_id uuid NOT NULL REFERENCES learning_enrollments ON DELETE RESTRICT,
 snapshot_id uuid NOT NULL, assessment_type text NOT NULL CHECK(assessment_type IN ('pre_test','quiz','post_test')),
 attempt_number integer NOT NULL CHECK(attempt_number>0), status text NOT NULL CHECK(status IN ('in_progress','submitted')),
 created_at timestamptz NOT NULL DEFAULT now(), submitted_at timestamptz,
 earned_points numeric(12,4), max_points numeric(12,4), score_percent numeric(5,2),
 UNIQUE(enrollment_id,assessment_type,attempt_number), UNIQUE(id,snapshot_id),
 FOREIGN KEY(snapshot_id,enrollment_id) REFERENCES assessment_snapshots(id,enrollment_id) ON DELETE RESTRICT,
 CHECK((status='in_progress' AND submitted_at IS NULL AND earned_points IS NULL AND max_points IS NULL AND score_percent IS NULL)
 OR (status='submitted' AND submitted_at IS NOT NULL AND earned_points IS NOT NULL AND max_points IS NOT NULL AND score_percent IS NOT NULL
 AND earned_points>=0 AND max_points>0 AND max_points<'Infinity'::numeric AND earned_points<=max_points AND score_percent BETWEEN 0 AND 100))
);
CREATE UNIQUE INDEX attempts_one_active ON assessment_attempts(enrollment_id,assessment_type) WHERE status='in_progress';
CREATE TABLE assessment_answers (
 attempt_id uuid NOT NULL, snapshot_id uuid NOT NULL, snapshot_question_id uuid NOT NULL,
 revision bigint NOT NULL CHECK(revision>0), saved_at timestamptz NOT NULL DEFAULT now(), earned_points numeric(12,4), is_correct boolean,
 PRIMARY KEY(attempt_id,snapshot_question_id), UNIQUE(attempt_id,snapshot_question_id,snapshot_id),
 FOREIGN KEY(attempt_id,snapshot_id) REFERENCES assessment_attempts(id,snapshot_id) ON DELETE RESTRICT,
 FOREIGN KEY(snapshot_id,snapshot_question_id) REFERENCES assessment_snapshot_questions(snapshot_id,id) ON DELETE RESTRICT,
 CHECK(earned_points IS NULL OR (earned_points>=0 AND earned_points<'Infinity'::numeric))
);
CREATE TABLE assessment_answer_options (
 attempt_id uuid NOT NULL, snapshot_question_id uuid NOT NULL, snapshot_id uuid NOT NULL, question_version_id uuid NOT NULL, option_id uuid NOT NULL,
 PRIMARY KEY(attempt_id,snapshot_question_id,option_id),
 FOREIGN KEY(attempt_id,snapshot_question_id,snapshot_id) REFERENCES assessment_answers(attempt_id,snapshot_question_id,snapshot_id) ON DELETE RESTRICT,
 FOREIGN KEY(question_version_id,option_id) REFERENCES assessment_options(question_version_id,id) ON DELETE RESTRICT,
 FOREIGN KEY(snapshot_id,snapshot_question_id,question_version_id) REFERENCES assessment_snapshot_questions(snapshot_id,id,question_version_id) ON DELETE RESTRICT
);
CREATE TABLE identity_sessions (
 id uuid PRIMARY KEY, token_hash bytea NOT NULL UNIQUE CHECK(octet_length(token_hash)=32), role text NOT NULL CHECK(role IN ('student','admin')),
 student_id uuid REFERENCES identity_students ON DELETE RESTRICT, admin_id uuid REFERENCES identity_admins ON DELETE RESTRICT,
 enrollment_id uuid, csrf_binding_hash bytea NOT NULL CHECK(octet_length(csrf_binding_hash)=32),
 created_at timestamptz NOT NULL DEFAULT now(), last_seen_at timestamptz NOT NULL DEFAULT now(),
 idle_expires_at timestamptz NOT NULL, absolute_expires_at timestamptz NOT NULL, revoked_at timestamptz,
 CHECK(idle_expires_at<=absolute_expires_at AND absolute_expires_at>created_at),
 CHECK((role='student' AND student_id IS NOT NULL AND enrollment_id IS NOT NULL AND admin_id IS NULL)
 OR (role='admin' AND admin_id IS NOT NULL AND student_id IS NULL AND enrollment_id IS NULL)),
 FOREIGN KEY(enrollment_id,student_id) REFERENCES learning_enrollments(id,student_id) ON DELETE RESTRICT
);

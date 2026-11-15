-- Context roots and immutable published content. Historical references use RESTRICT.
CREATE TABLE identity_schools (
 id uuid PRIMARY KEY, name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 300),
 normalized_name text NOT NULL UNIQUE, revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), archived_at timestamptz
);
CREATE INDEX schools_active_order ON identity_schools(normalized_name,id) WHERE archived_at IS NULL;
CREATE TABLE identity_students (id uuid PRIMARY KEY, display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 200), created_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE identity_admins (
 id uuid PRIMARY KEY, email text NOT NULL, display_name text NOT NULL,
 password_hash text NOT NULL CHECK (password_hash LIKE '$argon2id$%'), active boolean NOT NULL DEFAULT true,
 can_manage_content boolean NOT NULL DEFAULT false, can_manage_schools boolean NOT NULL DEFAULT false,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX admins_email_unique ON identity_admins(lower(email));
CREATE TABLE identity_admin_school_grants (
 admin_id uuid NOT NULL REFERENCES identity_admins ON DELETE RESTRICT,
 school_id uuid NOT NULL REFERENCES identity_schools ON DELETE RESTRICT, PRIMARY KEY(admin_id,school_id)
);
CREATE TABLE learning_lessons (
 id uuid PRIMARY KEY, state text NOT NULL CHECK (state IN ('draft','published','archived')),
 revision bigint NOT NULL DEFAULT 1 CHECK (revision>0), draft_payload jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE learning_lesson_versions (
 id uuid PRIMARY KEY, lesson_id uuid NOT NULL REFERENCES learning_lessons ON DELETE RESTRICT,
 version integer NOT NULL CHECK(version>0), title text NOT NULL, description text NOT NULL,
 published_at timestamptz NOT NULL DEFAULT now(), UNIQUE(lesson_id,version)
);
CREATE TABLE learning_sections (
 id uuid PRIMARY KEY, lesson_version_id uuid NOT NULL REFERENCES learning_lesson_versions ON DELETE RESTRICT,
 section_key uuid NOT NULL, position integer NOT NULL CHECK(position>0), title text NOT NULL, body text NOT NULL, required boolean NOT NULL,
 UNIQUE(lesson_version_id,section_key), UNIQUE(lesson_version_id,position), UNIQUE(lesson_version_id,id)
);
CREATE TABLE assessment_banks (
 id uuid PRIMARY KEY, kind text NOT NULL UNIQUE CHECK(kind IN ('paired','quiz')),
 revision bigint NOT NULL DEFAULT 1 CHECK(revision>0), state text NOT NULL CHECK(state IN ('draft','published','archived'))
);
CREATE TABLE assessment_questions (
 id uuid PRIMARY KEY, bank_id uuid NOT NULL REFERENCES assessment_banks ON DELETE RESTRICT,
 revision bigint NOT NULL DEFAULT 1 CHECK(revision>0), state text NOT NULL CHECK(state IN ('draft','published','archived')),
 draft_payload jsonb NOT NULL, position integer NOT NULL CHECK(position>0)
);
CREATE TABLE assessment_question_versions (
 id uuid PRIMARY KEY, question_id uuid NOT NULL REFERENCES assessment_questions ON DELETE RESTRICT,
 version integer NOT NULL CHECK(version>0), text text NOT NULL, type text NOT NULL CHECK(type IN ('single','multiple')),
 required boolean NOT NULL, scoring_rule text NOT NULL CHECK(scoring_rule='exact_set_v1'),
 max_points numeric(12,4) NOT NULL CHECK(max_points>0 AND max_points < 'Infinity'::numeric), UNIQUE(question_id,version)
);
CREATE TABLE assessment_options (
 id uuid PRIMARY KEY, question_version_id uuid NOT NULL REFERENCES assessment_question_versions ON DELETE RESTRICT,
 position integer NOT NULL CHECK(position>0), text text NOT NULL, is_correct boolean NOT NULL,
 UNIQUE(question_version_id,position), UNIQUE(question_version_id,id)
);
CREATE TABLE assessment_bank_versions (
 id uuid PRIMARY KEY, bank_id uuid NOT NULL REFERENCES assessment_banks ON DELETE RESTRICT,
 version integer NOT NULL CHECK(version>0), published_at timestamptz NOT NULL DEFAULT now(), UNIQUE(bank_id,version)
);
CREATE TABLE assessment_bank_items (
 bank_version_id uuid NOT NULL REFERENCES assessment_bank_versions ON DELETE RESTRICT,
 question_version_id uuid NOT NULL REFERENCES assessment_question_versions ON DELETE RESTRICT,
 position integer NOT NULL CHECK(position>0), PRIMARY KEY(bank_version_id,question_version_id), UNIQUE(bank_version_id,position)
);
CREATE TABLE learning_path_drafts (
 id uuid PRIMARY KEY, revision bigint NOT NULL DEFAULT 1 CHECK(revision>0), lesson_ids uuid[] NOT NULL,
 paired_bank_id uuid NOT NULL REFERENCES assessment_banks ON DELETE RESTRICT,
 quiz_bank_id uuid NOT NULL REFERENCES assessment_banks ON DELETE RESTRICT, updated_at timestamptz NOT NULL DEFAULT now()
);

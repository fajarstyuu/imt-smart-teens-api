-- Persist one mastery state per enrollment and pinned lesson version; every accepted answer
-- remains auditable so wrong retries are not overwritten by the final correct selection.
CREATE TABLE learning_lesson_quiz_attempts (
    id uuid PRIMARY KEY,
    enrollment_id uuid NOT NULL REFERENCES learning_enrollments(id) ON DELETE RESTRICT,
    lesson_version_id uuid NOT NULL REFERENCES learning_lesson_versions(id) ON DELETE RESTRICT,
    first_option_id uuid,
    attempt_count integer NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
    mastered boolean NOT NULL DEFAULT false,
    first_answered_at timestamptz,
    mastered_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (enrollment_id, lesson_version_id),
    CHECK ((attempt_count = 0) = (first_answered_at IS NULL)),
    CHECK (mastered = (mastered_at IS NOT NULL))
);
CREATE INDEX lesson_quiz_attempts_enrollment
    ON learning_lesson_quiz_attempts(enrollment_id, lesson_version_id);

CREATE TABLE learning_lesson_quiz_answers (
    attempt_id uuid NOT NULL REFERENCES learning_lesson_quiz_attempts(id) ON DELETE RESTRICT,
    revision bigint NOT NULL CHECK (revision > 0),
    option_id uuid NOT NULL,
    is_correct boolean NOT NULL,
    answered_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (attempt_id, revision)
);

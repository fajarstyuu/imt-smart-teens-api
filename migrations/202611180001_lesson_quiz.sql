-- Lesson-level mastery questions are immutable with their published lesson version.
-- NULL is retained only for historical pre-feature rows; new publication validates a complete quiz.
ALTER TABLE learning_lesson_versions
    ADD COLUMN quiz_payload jsonb;

ALTER TABLE learning_lesson_versions
    ADD CONSTRAINT lesson_version_quiz_payload_shape CHECK (
        quiz_payload IS NULL OR (
            jsonb_typeof(quiz_payload) = 'object'
            AND jsonb_typeof(quiz_payload->'text') = 'string'
            AND jsonb_typeof(quiz_payload->'options') = 'array'
            AND jsonb_array_length(quiz_payload->'options') = 2
            AND jsonb_typeof(quiz_payload->'correct_option_id') = 'string'
        )
    );

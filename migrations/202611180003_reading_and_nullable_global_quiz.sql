ALTER TABLE learning_lesson_progress
    ADD COLUMN reading_completed_at timestamptz;

-- The new learning sequence uses lesson-version-bound quizzes rather than a shared global bank.
-- Keep existing bank/version history intact while allowing new paths to omit the legacy reference.
ALTER TABLE learning_path_drafts
    ALTER COLUMN quiz_bank_id DROP NOT NULL;
ALTER TABLE learning_path_versions
    ALTER COLUMN quiz_bank_version_id DROP NOT NULL;

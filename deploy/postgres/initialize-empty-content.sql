-- Explicit operator operation, never invoked by normal server startup.
\set ON_ERROR_STOP on
BEGIN;
SELECT pg_advisory_xact_lock(718304291);
INSERT INTO assessment_banks(id,kind,state) VALUES(gen_random_uuid(),'paired','draft'),(gen_random_uuid(),'quiz','draft') ON CONFLICT(kind) DO NOTHING;
DO $$
BEGIN
    IF (SELECT count(*) FROM learning_path_drafts) > 1 THEN
        RAISE EXCEPTION 'Ambiguous content configuration; operator review required';
    END IF;
    IF EXISTS (SELECT 1 FROM learning_path_drafts d JOIN assessment_banks p ON p.id=d.paired_bank_id JOIN assessment_banks q ON q.id=d.quiz_bank_id WHERE p.kind <> 'paired' OR q.kind <> 'quiz') THEN
        RAISE EXCEPTION 'Invalid content root relationship; operator review required';
    END IF;
END $$;
INSERT INTO learning_path_drafts(id,lesson_ids,paired_bank_id,quiz_bank_id)
SELECT gen_random_uuid(),'{}',p.id,q.id FROM assessment_banks p,assessment_banks q
WHERE p.kind='paired' AND q.kind='quiz' AND NOT EXISTS(SELECT 1 FROM learning_path_drafts);
COMMIT;

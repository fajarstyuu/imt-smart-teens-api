-- Published rows are append-only. Draft roots remain editable; publications insert new versions.
CREATE FUNCTION reject_immutable_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 -- Preserve the established foreign-key deletion failure vocabulary for historical rows.
 IF TG_OP='DELETE' THEN
  RAISE EXCEPTION 'immutable published content' USING ERRCODE='23503';
 END IF;
 RAISE EXCEPTION 'immutable published content' USING ERRCODE='23514';
END;
$$;
DO $$
DECLARE table_name text;
BEGIN
 FOREACH table_name IN ARRAY ARRAY['learning_lesson_versions','learning_sections','assessment_question_versions','assessment_options','assessment_bank_versions','assessment_bank_items','learning_path_versions','learning_path_items','assessment_snapshots','assessment_snapshot_questions']
 LOOP
  EXECUTE format('CREATE TRIGGER immutable_update_delete BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION reject_immutable_change()',table_name);
 END LOOP;
END;
$$;
-- Extensions to referenced containers are changes too. Locking the parent conflicts with the
-- FK key-share lock taken by registration/publication inserts, preventing a concurrent append
-- from slipping through the reference check. Applications publish complete containers in one tx.
CREATE FUNCTION guard_pinned_extension() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE pinned boolean;
BEGIN
 CASE TG_TABLE_NAME
 WHEN 'assessment_options' THEN
  PERFORM id FROM assessment_question_versions WHERE id=NEW.question_version_id FOR UPDATE;
  SELECT EXISTS(SELECT 1 FROM assessment_bank_items WHERE question_version_id=NEW.question_version_id) INTO pinned;
 WHEN 'learning_sections' THEN
  PERFORM id FROM learning_lesson_versions WHERE id=NEW.lesson_version_id FOR UPDATE;
  SELECT EXISTS(SELECT 1 FROM learning_path_items WHERE lesson_version_id=NEW.lesson_version_id) INTO pinned;
 WHEN 'learning_path_items' THEN
  PERFORM id FROM learning_path_versions WHERE id=NEW.path_version_id FOR UPDATE;
  SELECT EXISTS(SELECT 1 FROM learning_enrollments WHERE path_version_id=NEW.path_version_id) INTO pinned;
 WHEN 'assessment_bank_items' THEN
  PERFORM id FROM assessment_bank_versions WHERE id=NEW.bank_version_id FOR UPDATE;
  SELECT EXISTS(SELECT 1 FROM learning_path_versions WHERE paired_bank_version_id=NEW.bank_version_id OR quiz_bank_version_id=NEW.bank_version_id) INTO pinned;
 WHEN 'assessment_snapshot_questions' THEN
  PERFORM id FROM assessment_snapshots WHERE id=NEW.snapshot_id FOR UPDATE;
  SELECT EXISTS(SELECT 1 FROM assessment_attempts WHERE snapshot_id=NEW.snapshot_id) INTO pinned;
 END CASE;
 IF pinned THEN RAISE EXCEPTION 'pinned container cannot be extended' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER pinned_extension BEFORE INSERT ON assessment_options FOR EACH ROW EXECUTE FUNCTION guard_pinned_extension();
CREATE TRIGGER pinned_extension BEFORE INSERT ON learning_sections FOR EACH ROW EXECUTE FUNCTION guard_pinned_extension();
CREATE TRIGGER pinned_extension BEFORE INSERT ON learning_path_items FOR EACH ROW EXECUTE FUNCTION guard_pinned_extension();
CREATE TRIGGER pinned_extension BEFORE INSERT ON assessment_bank_items FOR EACH ROW EXECUTE FUNCTION guard_pinned_extension();
CREATE TRIGGER pinned_extension BEFORE INSERT ON assessment_snapshot_questions FOR EACH ROW EXECUTE FUNCTION guard_pinned_extension();

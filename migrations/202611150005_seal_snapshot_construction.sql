-- Both paired AND quiz snapshots are sealed at enrollment commit, even before quiz creation.
-- Only the creating transaction may append snapshot questions. PostgreSQL xmin is a 32-bit
-- transaction ID; reduce the epoch-aware current xid8 modulo 2^32 for a wrap-safe comparison.
CREATE FUNCTION guard_snapshot_construction() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE creator bigint;
BEGIN
 SELECT xmin::text::bigint INTO creator FROM assessment_snapshots WHERE id=NEW.snapshot_id FOR UPDATE;
 IF creator IS DISTINCT FROM mod(pg_current_xact_id()::text::numeric,4294967296)::bigint THEN
  RAISE EXCEPTION 'snapshot construction is sealed' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER snapshot_construction BEFORE INSERT ON assessment_snapshot_questions FOR EACH ROW EXECUTE FUNCTION guard_snapshot_construction();

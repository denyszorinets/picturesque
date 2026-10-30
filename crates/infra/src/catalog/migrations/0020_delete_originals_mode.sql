-- A folder's own choice about deleting originals becomes one of three
-- (ADR-0017): NULL follows the setting, otherwise 'never', 'ask' or
-- 'immediately'. The 0/1 flag of 0019 is carried over and dropped.

ALTER TABLE import_source ADD COLUMN delete_originals_mode TEXT
    CHECK (delete_originals_mode IN ('never', 'ask', 'immediately'));
UPDATE import_source SET delete_originals_mode = CASE delete_originals
    WHEN 0 THEN 'never'
    WHEN 1 THEN 'ask'
END;
ALTER TABLE import_source DROP COLUMN delete_originals;
ALTER TABLE import_source RENAME COLUMN delete_originals_mode TO delete_originals;

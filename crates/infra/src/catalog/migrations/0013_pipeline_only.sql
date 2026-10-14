-- The import pipeline replaces import runs (ADR-0012).

-- Where a failed import job stopped: 'reading' or 'copying'.
ALTER TABLE import_job ADD COLUMN failed_stage TEXT;

DROP TABLE import_file;
DROP TABLE import_photo;
DROP TABLE import_run;

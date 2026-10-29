-- Deleting originals after a verified import (ADR-0017): a per-folder
-- choice (NULL follows the setting, 0 never, 1 ask) and a marker on the
-- jobs whose source files were deleted, so the offer comes from the
-- catalog and a cancelled or interrupted run resumes with what is left.

ALTER TABLE import_source ADD COLUMN delete_originals INTEGER CHECK (delete_originals IN (0, 1));
ALTER TABLE import_job ADD COLUMN source_removed_at TEXT;

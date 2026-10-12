-- Sidecar bookkeeping: a revision counter per decision (a sidecar is up to
-- date when it reflects the latest revision), and decisions read from
-- existing sidecars during import.

ALTER TABLE decision ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;
ALTER TABLE decision ADD COLUMN sidecar_revision INTEGER;

ALTER TABLE import_photo ADD COLUMN decision TEXT CHECK (decision IS NULL OR json_valid(decision));

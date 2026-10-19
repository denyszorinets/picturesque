-- Photos whose sidecars carry the auto-pick colour label, set when a
-- folder is finished (ADR-0013, docs/features/folders.rst). Changing it
-- bumps the decision's revision so the sidecar is rewritten.

ALTER TABLE decision ADD COLUMN auto_label INTEGER NOT NULL DEFAULT 0;

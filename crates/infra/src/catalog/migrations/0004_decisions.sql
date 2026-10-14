-- Culling decisions, and indexes for browsing queries.

CREATE TABLE decision (
    photo_id            INTEGER PRIMARY KEY REFERENCES photo (id) ON DELETE CASCADE,
    flag                TEXT NOT NULL DEFAULT 'unflagged' CHECK (flag IN ('unflagged', 'pick', 'reject')),
    rating              INTEGER NOT NULL DEFAULT 0 CHECK (rating BETWEEN 0 AND 5),
    updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- When the XMP sidecar last reflected this decision.
    sidecar_written_at  TEXT
);

CREATE INDEX decision_flag ON decision (flag, rating);

DROP INDEX photo_captured_local;
CREATE INDEX photo_captured ON photo (captured_local, id);
CREATE INDEX photo_camera ON photo (camera_make, camera_model);
CREATE INDEX photo_lens ON photo (lens);
CREATE INDEX photo_place ON photo (place_country_code, place_city);

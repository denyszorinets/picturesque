-- Photos and their files (docs/architecture/data.rst).

CREATE TABLE photo (
    id                  INTEGER PRIMARY KEY,
    -- Local wall-clock capture time, ISO 8601 without offset; sorts as text.
    captured_local      TEXT,
    -- Seconds east of UTC, when the file records it.
    captured_offset_s   INTEGER,
    camera_make         TEXT,
    camera_model        TEXT,
    lens                TEXT,
    iso                 INTEGER,
    f_number            REAL,
    exposure_num        INTEGER,
    exposure_den        INTEGER,
    focal_length_mm     REAL,
    gps_lat             REAL,
    gps_lon             REAL,
    place_city          TEXT,
    place_region        TEXT,
    place_country       TEXT,
    place_country_code  TEXT,
    -- EXIF orientation value 1-8.
    orientation         INTEGER NOT NULL DEFAULT 1 CHECK (orientation BETWEEN 1 AND 8),
    width               INTEGER,
    height              INTEGER,
    imported_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX photo_captured_local ON photo (captured_local);

CREATE TABLE asset (
    id          INTEGER PRIMARY KEY,
    photo_id    INTEGER NOT NULL REFERENCES photo (id) ON DELETE CASCADE,
    role        TEXT NOT NULL CHECK (role IN ('raw', 'image', 'sidecar')),
    format      TEXT NOT NULL,
    -- Relative to the Clean library root, '/'-separated.
    path        TEXT NOT NULL UNIQUE,
    size        INTEGER NOT NULL CHECK (size >= 0),
    -- BLAKE3, 32 bytes.
    hash        BLOB NOT NULL CHECK (length(hash) = 32)
);

CREATE INDEX asset_photo ON asset (photo_id);

-- Each RAW or image content is catalogued once (ADR-0008). Sidecars may repeat.
CREATE UNIQUE INDEX asset_media_hash ON asset (hash) WHERE role <> 'sidecar';

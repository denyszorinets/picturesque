-- Results of background analyzers (technical scores, perceptual hashes,
-- later AI models): one row per photo and analyzer. A photo needs an
-- analyzer again when the stored version differs from the analyzer's.

CREATE TABLE analysis (
    photo_id    INTEGER NOT NULL REFERENCES photo (id) ON DELETE CASCADE,
    analyzer    TEXT NOT NULL,
    version     INTEGER NOT NULL,
    result      TEXT NOT NULL CHECK (json_valid(result)),
    analyzed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (photo_id, analyzer)
) WITHOUT ROWID;

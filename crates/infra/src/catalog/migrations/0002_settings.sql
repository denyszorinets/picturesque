-- Application settings as JSON documents keyed by name.

CREATE TABLE setting (
    key     TEXT PRIMARY KEY,
    value   TEXT NOT NULL CHECK (json_valid(value))
);

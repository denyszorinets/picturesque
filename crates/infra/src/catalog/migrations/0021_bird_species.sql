-- The bird species the user chose for a photo with Bird ID
-- (docs/features/bird-id.rst): names as the model's labels give them, and
-- the eBird species code.

CREATE TABLE bird_species (
    photo_id   INTEGER PRIMARY KEY REFERENCES photo (id) ON DELETE CASCADE,
    common     TEXT NOT NULL,
    scientific TEXT NOT NULL,
    code       TEXT NOT NULL
);

-- A photo can show several bird species (Find birds,
-- docs/features/bird-id.rst): one row per photo and species, keeping the
-- species chosen so far.

CREATE TABLE bird_species_many (
    photo_id   INTEGER NOT NULL REFERENCES photo (id) ON DELETE CASCADE,
    common     TEXT NOT NULL,
    scientific TEXT NOT NULL,
    code       TEXT NOT NULL,
    PRIMARY KEY (photo_id, code)
);

INSERT INTO bird_species_many (photo_id, common, scientific, code)
    SELECT photo_id, common, scientific, code FROM bird_species;
DROP TABLE bird_species;
ALTER TABLE bird_species_many RENAME TO bird_species;

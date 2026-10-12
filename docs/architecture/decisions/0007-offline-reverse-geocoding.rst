ADR-0007: Offline Reverse Geocoding
===================================

:Status: Accepted
:Date: 2026-10-12

Context
-------

Path templates and filters use place names (city, region, country) derived
from GPS coordinates in photo metadata. Picturesque is private and
offline-first. An import of thousands of photos needs thousands of lookups,
quickly. City-level precision is enough for folder names and filters.

Decision
--------

Resolve coordinates offline using the **GeoNames** ``cities500`` dataset
(places with population of at least 500), with admin-region and country
names, packaged as a compact asset in the application.

- A k-d tree built at startup answers nearest-place lookups.
- The dataset is licensed CC BY 4.0; attribution appears in the About
  screen and documentation.
- The geocoder sits behind a port, so an optional online provider could be
  added later.

Alternatives Considered
-----------------------

Online geocoding (Nominatim/OpenStreetMap)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Street- and neighbourhood-level detail.

Disadvantages:

- Sends photo locations to a third party.
- The public service is limited to one request per second; slow for large
  imports and requires network access.

Offline OpenStreetMap boundaries
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Exact administrative boundaries instead of nearest city.

Disadvantages:

- Hundreds of megabytes of data and point-in-polygon processing.

Decision Criteria
-----------------

Privacy, speed for bulk imports, asset size, precision needed for folder
names.

Rationale
---------

Nearest-city lookup against a few tens of megabytes of data is instant,
private and accurate enough for folder names. Nearest-city can be wrong near
borders or in sparsely populated areas; that is acceptable for organizing
photos and is documented.

Consequences
------------

Positive
~~~~~~~~

- No network access; no rate limits.
- Microsecond lookups.

Negative
~~~~~~~~

- Larger installer.
- City-level precision only; nearest city may be across a border.

Risks
~~~~~

- Dataset updates require an app release.

Follow-up
~~~~~~~~~

- Implement the geocoder (:issue:`17`).

Validation
----------

:issue:`17` tests known coordinates and measures lookup time.

References
----------

- `GeoNames <https://www.geonames.org/>`_

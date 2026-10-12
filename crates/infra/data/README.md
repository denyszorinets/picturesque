# Bundled data

## `geonames-cities500.tsv.gz`

Reverse-geocoding dataset (ADR-0007), built by `scripts/build_geonames.py`
from [GeoNames](https://www.geonames.org/) `cities500.zip`,
`admin1CodesASCII.txt` and `countryInfo.txt`.

**License: CC BY 4.0. Attribution: GeoNames (geonames.org).**

Populated places with at least 500 inhabitants, excluding neighbourhoods
(`PPLX`) and historical, abandoned, destroyed or religious places. Format:

```
C <tab> country code <tab> country name
P <tab> name <tab> region <tab> country code <tab> lat <tab> lon <tab> population
```

To refresh: `python3 -I scripts/build_geonames.py <download-dir>`.

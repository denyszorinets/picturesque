Third-Party Data
================

Data bundled with Picturesque and its licences.

GeoNames
--------

Place names for photos with GPS coordinates
(:ref:`features-import-places`).

:Source: `GeoNames <https://www.geonames.org/>`_ ``cities500``, admin1 and
   country names
:Licence: `Creative Commons Attribution 4.0
   <https://creativecommons.org/licenses/by/4.0/>`_
:Attribution: Place names from GeoNames (geonames.org), licensed CC BY 4.0.
:In the repository: ``crates/infra/data/geonames-cities500.tsv.gz``,
   rebuilt with ``scripts/build_geonames.py``

Heroicons
---------

The icons of the interface, bundled into the app so it works offline.

:Source: `Heroicons <https://heroicons.com/>`_ by Tailwind Labs, the
   20 px solid set (npm package ``heroicons``)
:Licence: `MIT <https://github.com/tailwindlabs/heroicons/blob/master/LICENSE>`_
:Attribution: Copyright (c) Tailwind Labs, Inc.
:In the repository: imported by ``ui/src/lib/components/Icon.svelte``;
   add an icon there to use it

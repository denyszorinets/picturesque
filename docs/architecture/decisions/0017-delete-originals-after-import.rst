ADR-0017: Optional Deletion of Originals After a Verified Import
================================================================

:Status: Accepted; amends
   :doc:`ADR-0008 <0008-non-destructive-verified-import>` and
   :doc:`ADR-0014 <0014-optional-source-deletion>`
:Date: 2026-10-29

Context
-------

:doc:`ADR-0008 <0008-non-destructive-verified-import>` made import sources
read-only, and :doc:`ADR-0014 <0014-optional-source-deletion>` allowed
deleting originals only in folder sessions. Users who import from a card
still clear it by hand afterwards, without knowing which files were
copied.

Constraints:

- Losing a photo is the worst possible failure; a card is often the only
  copy until it is cleared.
- Importing itself must stay read-only and safe.
- Cards have no reliable trash (see ADR-0014).

Decision
--------

Picturesque may delete the originals of an import folder, only after the
import and only when the user asks.

- **Optional, off by default.** The setting ``delete_after_import`` is
  ``never`` (default) or ``ask``. Each import folder can override it
  (``import_source.delete_originals``; NULL follows the setting).
- **A separate, confirmed step.** With ``ask``, once a folder has no queued
  photos, Activity offers **Delete originals…** (from the catalog only).
  The dialog shows photos, files, size, duplicates and photos changed since
  the import (which stay), and says deletion is permanent. Confirming
  starts a background run: one at a time, cancellable between photos,
  reporting ``originals_progress``.
- **Both sides verified, per photo, right before deleting.** The import
  job must be unchanged; each original that still exists must be inside
  the import folder (paths with ``..`` are refused) and unchanged since
  the import by size and modification time, media files included. Each
  media original must hash to a media asset of that photo (hash and size)
  in the library, and the library file must hash to the hash recorded. A
  file equal to the library's own copy is never deleted. Otherwise the
  whole photo stays and is reported. Just before each file is removed it
  is looked at once more; if it changed, the rest of that photo stays.
- **Accepted limit: sidecars.** XMP sidecars are checked by size and
  modification time only, since the library rewrites its own sidecar and
  there is no hash to compare; media files are compared by hash.
- **Duplicates included, same rule.** A RAW+JPEG pair with only one file
  in the library stays.
- **Order and failure.** Media files first, then sidecars; at the first
  failure the rest of that photo stays and is reported.
- **Resumable, never silent.** Each fully deleted photo is recorded
  (``import_job.source_removed_at``), so a cancelled or interrupted run
  continues with the rest and the offer disappears. After a restart
  nothing is deleted without a new confirmation. Photos imported after the
  plan are not part of it (``through_job_id``). Importing a changed file
  with the same name clears the marker.
- **Overlapping folders refused.** If the library folder and the import
  folder overlap (one contains the other, e.g. the library folder was
  changed later), nothing is deleted.
- **Folder removed during a run.** The run stops (``cancelled``).
- **Already gone.** Photos whose originals are all gone are recorded as
  done and reported as ``gone``, not as deleted.
- **Refused** for a folder culled in place (its source is the library), a
  folder that is not writable (a locked card), and while photos from the
  folder are still being imported.
- **Permanent, not to the trash**, for the reasons of ADR-0014.

Alternatives Considered
-----------------------

Delete during copying ("move")
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Rejected before (ADR-0014) because it mixes a safe operation with an
unsafe one. This decision differs: deleting is a separate step, after the
copy is verified, confirmed by the user, and both the original and the
library copy are verified again right before each photo.

Move to the trash
~~~~~~~~~~~~~~~~~

Rejected for the reasons in ADR-0014: space on cards stays used and
behaviour differs by platform.

Automatic deletion
~~~~~~~~~~~~~~~~~~

Deleting without a confirmation after every import. Rejected for now:
too risky before the confirmed mode has proved itself; ``delete_after_import``
can gain a value later.

Decision Criteria
-----------------

Data safety, then fewer manual steps for users clearing cards.

Rationale
---------

A separate confirmed step with verification of both sides at the moment of
deletion is safer than clearing a card by hand, and the default ``never``
keeps ADR-0008's behaviour for everyone who does not opt in.

Consequences
------------

Positive
~~~~~~~~

- Importing and clearing a card can be done in Picturesque, each step
  verified.

Negative
~~~~~~~~

- More code that deletes photos; no undo.
- ADR-0008's "sources are never deleted" is no longer absolute; it holds
  for importing itself.

Risks
~~~~~

- A failing destination disk could pass verification and fail later;
  hashing the library file right before deleting narrows that window.
- Users who choose ``ask`` may confirm without reading; the dialog states
  that deletion is permanent.

Validation
----------

The tests in ``crates/app/src/originals/tests.rs`` and
``crates/infra/tests/pipeline.rs`` cover: deleting only verified photos; a
changed or missing library copy, a changed job or changed sidecars keeping
the photo; duplicates and incomplete RAW+JPEG pairs; failure in the middle
of a photo; cancel and resume; photos imported after the plan; and the
refused cases.

References
----------

- :doc:`ADR-0008 <0008-non-destructive-verified-import>`
- :doc:`ADR-0014 <0014-optional-source-deletion>`
- :issue:`244`

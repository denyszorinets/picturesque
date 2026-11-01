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

Amendment: delete right after each verified copy
------------------------------------------------

Context
~~~~~~~

Deleting after the import cannot import a library bigger than the free
disk space (for example 900 GB of photos on a 1 TB disk): the originals
are only removed once everything is copied.

Decision
~~~~~~~~

``delete_after_import`` and each folder's own choice gain a third value,
``immediately`` (the folder choice is now ``default``, ``never``, ``ask``
or ``immediately``).

- Each original is deleted as soon as its photo is imported or recognised
  as a duplicate (stage ``imported`` or ``duplicate``) and verified, one
  photo at a time while the import is still running. No offer or dialog
  follows. Nothing is deleted before the catalog row exists.
- The per-photo checks are those above, in the same order: media first,
  then sidecars; a photo stays whole if any check fails.
- A file that fails stays, and the reason goes to the import log as a
  warning, once per folder until a deletion works again.
- Not applied for a folder culled in place, when the library and import
  folders overlap, and for photos that failed or are not yet imported.
- Durable before deleted: the re-read of the library copy may come from
  memory, so before any original is deleted (in both routes) the copy is
  flushed with its folders up to the library root
  (``FileSystem::sync_durable``). If that fails the original stays. The
  catalog commits with ``synchronous=FULL``, so a power loss or a pulled
  disk seconds after a deletion cannot lose the photo.
- A folder that cannot be deleted from (a locked card) is checked once
  and left alone for a minute, instead of reading every photo twice for
  nothing.
- Switching it on (wizard, Settings, Activity, add-folder select) needs a
  confirmation: permanent, no trash, no undo, the library disk is the
  only copy. Cancelling leaves the choice unchanged. Folders set to
  ``immediately`` still show **Delete originals…** for leftovers.
- The pipeline has an ``after_import(job_id)`` hook, set in ``api.rs`` to
  ``OriginalsService::after_import`` and called after ``process()`` for
  each job. The pipeline names no deletion logic; layering is unchanged
  (:doc:`ADR-0003 <0003-layered-cargo-workspace>`).
- Migration ``0020_delete_originals_mode`` turns
  ``import_source.delete_originals`` from INTEGER 0/1 into TEXT (NULL,
  ``never``, ``ask``, ``immediately``; 0 became ``never``, 1 ``ask``).
  ``import_job.source_removed_at`` is unchanged and is set when the
  originals are deleted. The API accepts ``immediately``;
  ``ImportSource.delete_originals`` is a string or null instead of a
  boolean or null. No new commands or events.

Consequences
~~~~~~~~~~~~

- Libraries larger than the free disk space can be imported.
- Each photo is read again (original and library copy hashed) before its
  original is deleted, so importing is slower.
- The original may be the only other copy until the library copy is
  verified; that is why the same checks run before every deletion.

References
----------

- :doc:`ADR-0008 <0008-non-destructive-verified-import>`
- :doc:`ADR-0014 <0014-optional-source-deletion>`
- :issue:`244`

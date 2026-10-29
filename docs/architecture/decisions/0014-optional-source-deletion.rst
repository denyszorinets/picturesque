ADR-0014: Optional Deletion of Source Files After a Verified Copy
=================================================================

:Status: Accepted
:Date: 2026-10-19

See also :doc:`ADR-0017 <0017-delete-originals-after-import>`, which
extends optional deletion to imported folders.

Context
-------

:doc:`ADR-0008 <0008-non-destructive-verified-import>` made sources
read-only: Picturesque never deletes a photo, and users delete their
originals themselves once satisfied.

Folder sessions (:doc:`ADR-0013 <0013-folder-sessions>`) let users cull a
memory card in place, then copy the keepers elsewhere (:issue:`170`). Users
asked to finish that in one pass: "copy to selected folder", then clear the
card. Doing this by hand means finding which files on the card were copied,
which is error-prone exactly where a mistake loses photos.

Constraints:

- Losing a photo is still the worst possible failure.
- A card is often the only copy until it is cleared.
- Users want different things: some delete only what was copied, some
  also want the rejects gone, some never want the app to delete anything.
- Cards are usually FAT or exFAT; there is no recycle bin on Windows
  removable drives, and a trash folder on the card keeps the space used.

Decision
--------

Picturesque may delete original files in a folder session, only when the
user asks, and only under these rules.

- **A setting decides what may be deleted** (``delete_after_copy``):

  - ``never``: nothing; the action is not offered.
  - ``verified`` (default): only photos whose copy was verified.
  - ``verified_and_rejects``: those, plus photos the user rejected.

- **Copies are recorded.** Copying keepers out of a folder session records,
  per photo, where each file went and its BLAKE3 hash once the copy was
  verified (the same copy-then-verify as ADR-0008 and export).
- **Verified again before deleting.** Just before a source file is
  deleted, its recorded copy is hashed again and must still match; a
  photo whose copy is missing or changed is kept and reported.
- **Explicit and confirmed.** Deleting is a separate action, never a side
  effect of copying. Its confirmation states how many photos and how much
  space, and lists rejects separately when they are included. Unflagged
  photos that were not copied are never deleted.
- **Whole photos.** A photo's files (RAW, JPEG, sidecars) are deleted
  together; if one cannot be deleted, the rest are kept and the photo is
  reported.
- **Permanent, not to the trash.** Each deleted file has a verified copy,
  or is a reject the user chose to delete; a trash folder on a card keeps
  the space used, which defeats clearing it, and is not available on
  Windows removable drives. The confirmation says that deletion is
  permanent.
- **Only folder sessions.** The library is never cleared by Picturesque;
  ADR-0008 still holds for imports.

Alternatives Considered
-----------------------

Never delete (keep ADR-0008 as is)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- No way for Picturesque to lose a photo.

Disadvantages:

- Users clear cards by hand or by formatting, without knowing what was
  copied; the safety stays in the app only if the app does the clearing.

Move to the trash
~~~~~~~~~~~~~~~~~

Advantages:

- Undo is possible.

Disadvantages:

- On a card the space stays used until the trash is emptied, and Windows
  has no recycle bin on removable drives, so behaviour would differ by
  platform and by drive. The re-verified copy is a stronger safety net.

Delete as part of copying ("move")
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- One step.

Disadvantages:

- Mixes a safe operation with an unsafe one; a failed or cancelled copy
  leaves the user unsure what was deleted.

Decision Criteria
-----------------

Data safety, then matching what users already do with cards, then the
number of steps.

Rationale
---------

Deleting only what has a copy, checked again at the moment of deletion, is
safer than what users do by hand today. Keeping deletion a separate,
confirmed action with a conservative default means no photo is deleted
by surprise, and ``never`` keeps ADR-0008's behaviour for users who want
it.

Consequences
------------

Positive
~~~~~~~~

- A card can be culled, copied and cleared in Picturesque, with each step
  verified.

Negative
~~~~~~~~

- Picturesque now has code that deletes photos; it needs thorough tests,
  including failures in the middle.
- No undo: deleted files are gone.

Risks
~~~~~

- A destination on a failing disk could pass verification and fail later;
  re-verifying right before deletion narrows that window.
- Users may pick ``verified_and_rejects`` and regret a reject; the
  confirmation lists rejects separately.

Follow-up
~~~~~~~~~

- Copy keepers from a folder session (:issue:`170`).
- Clear the folder after copying (:issue:`171`).

Validation
----------

Tests cover: deleting only verified photos; a copy changed or removed
after copying keeps its source; rejects only with the setting; a file that
cannot be deleted keeps the rest of its photo; ``never`` deletes nothing.

References
----------

- :doc:`ADR-0008 <0008-non-destructive-verified-import>`
- :doc:`ADR-0013 <0013-folder-sessions>`
- :issue:`169`

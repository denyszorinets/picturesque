.. _features-compare:

Compare
=======

The compare view shows two or four photos side by side, so you can pick
the best shot of a series and reject the rest, using only the keyboard.

Opening it
----------

In the Library, press **C** (or click **Compare** above the timeline):

- with **two or more photos selected**, they are compared in timeline
  order;
- otherwise, the **series** of the selected photo (or the photo in the
  loupe) is compared, best-ranked photo first (:ref:`features-series`),
  each with its ranking score.

The view starts in 4-up when there are more than two photos, else 2-up,
with the selected photo active. The active photo has a white outline;
every key below applies to it.

A RAW+JPEG pair is marked **RAW+JPEG** under its pane and shows the file
:ref:`pair_view <reference-settings-pair-view>` chooses.

Keys
----

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Key
     - Action
   * - ← →, Home / End
     - Make the previous / next, first / last photo active. When there are
       more photos than panes, the panes scroll to keep the active one in
       view.
   * - L
     - Switch between 2-up and 4-up
   * - P, X, U
     - Pick, reject, unflag
   * - 0 – 5
     - Rate (0 clears)
   * - B
     - Make the photo the best of its series; again to withdraw the choice
   * - Z
     - Zoom every pane to 100%, or back to fitted
   * - Shift + arrows
     - Pan all zoomed panes together
   * - Ctrl+Z, Ctrl+Shift+Z
     - Undo, redo (shared with the timeline)
   * - Esc
     - Leave zoom; close the view

With the mouse: click a pane to make it active, double-click a point to
zoom there, drag or scroll a zoomed pane to pan. Zoom and pan are
**synchronized**: every pane shows the same part of its photo (as a
fraction of the photo's size), so you can compare focus on the same eye
in each shot.

Rejected photos are dimmed and labelled; picks, stars and the best photo
are shown under each pane. Closing the view selects the active photo in
the timeline, and every change is already saved.

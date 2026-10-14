ADR-0010: Dual Transport: Tauri IPC and Development HTTP
========================================================

:Status: Accepted
:Date: 2026-10-12

Context
-------

Development happens inside a container with no display. The Tauri window
(WebKitGTK on Linux) cannot be shown there, yet the UI must be developed and
verified against the real backend with real photos, including automated
browser tests. Unit tests of Rust code alone do not exercise the UI.

Decision
--------

Expose the application's commands through two transports that share one
definition:

- **Tauri IPC** in ``picturesque-desktop``: the shipped application.
- **HTTP** in ``picturesque-devserver`` (axum): development and test only,
  never shipped. Commands are JSON ``POST`` requests, events are a
  server-sent event stream, and images are served on routes that match the
  desktop custom protocol.

Commands, their argument and result types, and events are defined once in
``picturesque-app``. Both entry points are thin adapters. In the UI, a single
``api`` module chooses the transport at startup (Tauri when running inside
the desktop shell, HTTP otherwise).

The dev server binds to ``127.0.0.1`` only.

Alternatives Considered
-----------------------

Mock the backend in the browser
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- No extra server.

Disadvantages:

- Tests the UI against fake data; integration bugs surface late.

Run Tauri under a virtual display (Xvfb) in the container
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advantages:

- Tests the real shell.

Disadvantages:

- Slow and fragile for day-to-day development; browser tooling (Playwright)
  works far better with a normal browser.

Decision Criteria
-----------------

Ability to verify the real UI against the real backend without a display,
maintenance cost, risk of the two transports diverging.

Rationale
---------

Because commands are defined once and both entry points only adapt them,
the extra transport is small and cannot drift in behaviour. Development uses
standard browser tooling. The real Tauri window is still exercised by a
smoke test in CI under Xvfb (:issue:`54`).

Consequences
------------

Positive
~~~~~~~~

- UI work and end-to-end browser tests run in the container.
- Command definitions are framework-independent.

Negative
~~~~~~~~

- A second entry point to maintain.
- Platform web-view differences are not caught by browser tests.

Risks
~~~~~

- Exposing file-system operations over HTTP is dangerous if the server were
  reachable from the network; it binds to loopback only and is not part of
  release builds.

Follow-up
~~~~~~~~~

- Build the dev server and UI transport (:issue:`8`).

Validation
----------

:issue:`8` includes a Playwright test of the UI against the dev server.

References
----------

- :doc:`0002-tauri-2-and-svelte-5`
- :doc:`/architecture/components`

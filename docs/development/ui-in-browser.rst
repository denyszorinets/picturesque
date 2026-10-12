Running the UI in a Browser
===========================

The devcontainer has no display, so the desktop window cannot open there.
Instead, the real UI runs in a browser and talks to the real backend
through the development HTTP server, ``picturesque-devserver``
(:doc:`ADR-0010 </architecture/decisions/0010-dual-transport-tauri-ipc-and-dev-http>`).
The server is for development and tests only and is never shipped.

.. mermaid::

   flowchart LR
     browser[Browser] -- "/ (UI)" --> vite[Vite dev server<br/>127.0.0.1:5173]
     vite -- "/api/* (proxy)" --> dev[picturesque-devserver<br/>127.0.0.1:5174]
     dev --> api[picturesque-app Api]

Start both servers
------------------

.. code-block:: bash

   make dev-web

Then open http://127.0.0.1:5173. ``Ctrl+C`` stops both servers. To reach
the page from the host, forward port 5173 from the devcontainer (VS Code
does this automatically).

The dev server listens on ``127.0.0.1:5174``. Set
``PICTURESQUE_DEVSERVER_ADDR`` to change it; non-loopback addresses are
refused, because the server exposes file-system operations. If you change
the port, update the ``/api`` proxy in ``ui/vite.config.ts`` to match.

HTTP API
--------

The server mirrors the Tauri command API exactly:

``GET /api/health``
   Returns ``ok``.

``POST /api/commands/{name}``
   Runs the command ``name``. The request body is the JSON arguments, or
   empty when the command takes none. A successful call returns the JSON
   result with status 200. A failed call returns an error body:

   .. code-block:: json

      { "code": "unknown_command", "message": "unknown command `nope`" }

   .. list-table::
      :header-rows: 1

      * - ``code``
        - HTTP status
      * - ``invalid_arguments``
        - 400
      * - ``unknown_command``
        - 404
      * - ``internal``
        - 500

For example:

.. code-block:: console

   $ curl -s -X POST http://127.0.0.1:5174/api/commands/app_info
   {"name":"Picturesque","version":"0.1.0"}

End-to-end tests
----------------

Browser tests live in ``ui/e2e/`` and use Playwright. They start the dev
server and Vite automatically (or reuse ones already running locally):

.. code-block:: bash

   make ui-deps   # once: npm packages and the Playwright Chromium build
   make e2e

On failure, Playwright keeps a trace in ``ui/test-results/``; open it with
``npx playwright show-trace <path>/trace.zip``.

Playwright browser tools for Claude Code
----------------------------------------

``.mcp.json`` configures the Playwright MCP server to use headless
Chromium. Its version is pinned because each release expects a specific
Chromium build. After changing the version, install the matching browser
with that release's ``playwright-core`` (``npx playwright install
chromium`` run from the same package) and restart the session.

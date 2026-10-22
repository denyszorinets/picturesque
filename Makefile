.PHONY: ort-dylib screenshots dist dist-deb dist-rpm dist-appimage dist-linux dist-macos dist-windows smoke help format format-check test lint build doc check clean ui-deps ui-check e2e bindings-check dev-web app-dev docs docs-serve docs-check

DOCS_DIR := docs
SPHINX := uv run --project $(DOCS_DIR) --frozen
# Extra Cargo flags; CI sets CARGO_FLAGS=--locked.
CARGO_FLAGS ?=

help:
	@echo "Available targets:"
	@echo "  format        Format Rust and front-end code"
	@echo "  format-check  Check Rust formatting"
	@echo "  test          Run Rust tests (unit, integration, doc)"
	@echo "  lint          Run clippy with warnings as errors"
	@echo "  build         Build all workspace targets"
	@echo "  doc           Build Rust API docs with warnings as errors"
	@echo "  check         format-check + lint + test + build + ui-check"
	@echo "  ui-deps       Install front-end dependencies and the Playwright browser"
	@echo "  ort-dylib     Make ONNX Runtime's shared library (Linux x86-64; ORT_PROFILE=release)"
	@echo "  ui-check      Front-end type check, lint, format check and tests"
	@echo "  e2e           Browser end-to-end tests against the dev server (Playwright)"
	@echo "  bindings-check Fail if generated TypeScript API types are out of date"
	@echo "  dev-web       Run the UI in a browser against the dev server (no display needed)"
	@echo "  screenshots   Rebuild the website and docs screenshots from a CC0 demo library"
	@echo "  app-dev       Run the desktop app with hot reload (needs a display)"
	@echo "  smoke         Build the desktop app and smoke-test it on Xvfb (tauri-driver)"
	@echo "  dist          Build the installers for this OS into target/release/bundle"
	@echo "  dist-deb      Build the .deb (on Debian/Ubuntu)"
	@echo "  dist-rpm      Build the .rpm (on Fedora)"
	@echo "  dist-appimage Build the .AppImage (on Linux)"
	@echo "  dist-linux    Build the .deb, .rpm and .AppImage"
	@echo "  dist-macos    Build the .dmg (on macOS, needs Homebrew libraries)"
	@echo "  dist-windows  Build the .msi and setup .exe (on Windows, after scripts/setup-windows-libvips.sh)"
	@echo "  clean         Remove build artifacts"
	@echo "  docs          Build documentation into docs/_build/html"
	@echo "  docs-serve    Live-reloading documentation preview on http://127.0.0.1:8000"
	@echo "  docs-check    Strict documentation build and link check"

format:
	cargo fmt --all
	npm run format -w ui

format-check:
	cargo fmt --all --check

# ONNX Runtime as a shared library, loaded at first use on Linux x86-64
# (ADR-0015); a no-op elsewhere. ORT_PROFILE is debug or release.
ORT_PROFILE ?= debug
ort-dylib:
	scripts/ort-dylib.sh $(ORT_PROFILE)

test: ort-dylib
	cargo test --workspace --all-targets $(CARGO_FLAGS)
	cargo test --workspace --doc $(CARGO_FLAGS)

lint:
	cargo clippy --workspace --all-targets --all-features $(CARGO_FLAGS) -- -D warnings

build: ort-dylib
	cargo build --workspace --all-targets $(CARGO_FLAGS)

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

check: format-check lint test build ui-check

ui-deps:
	npm ci
	npm exec -w ui -- playwright install chromium

ui-check:
	npm run check -w ui
	npm run lint -w ui
	npm run test -w ui

e2e: ort-dylib
	npm run e2e -w ui

# ts-rs regenerates ui/src/lib/api/generated when tests run.
bindings-check: test
	git diff --exit-code -- ui/src/lib/api/generated
	@test -z "$$(git ls-files --others --exclude-standard -- ui/src/lib/api/generated)" || \
		(echo "untracked generated bindings; commit them" && exit 1)

dev-web: ort-dylib
	cargo build -p picturesque-devserver
	trap 'kill 0' EXIT INT TERM; \
		cargo run --quiet -p picturesque-devserver & \
		npm run dev -w ui

# Screenshots for the website and docs (docs/development/website.rst): a
# demo card from CC0 photos, imported into an empty dev server whose models
# are kept between runs.
SHOTS := target/screenshots
screenshots:
	scripts/ort-dylib.sh release
	cargo build --release -p picturesque-devserver
	python3 scripts/screenshots/build-demo.py $(SHOTS)/card
	rm -rf $(SHOTS)/data/catalog.sqlite* $(SHOTS)/data/cache $(SHOTS)/library $(SHOTS)/Iceland
	mkdir -p $(SHOTS)/data $(SHOTS)/library
	@# Stops only the two servers it started (kill 0 would also stop the
	@# caller, such as CI).
	PICTURESQUE_DATA_DIR=$(SHOTS)/data target/release/picturesque-devserver & dev=$$!; \
		(cd ui && exec node ../node_modules/vite/bin/vite.js >/dev/null) & vite=$$!; \
		trap 'kill $$dev $$vite 2>/dev/null' EXIT INT TERM; \
		until curl -sf http://127.0.0.1:5174/api/health >/dev/null && \
			curl -sf http://127.0.0.1:5173 >/dev/null; do sleep 1; done; \
		node scripts/screenshots/capture.mjs $(SHOTS)/card $(SHOTS)/library $(SHOTS)/Iceland

# The real app on a virtual display; WAYLAND_DISPLAY is cleared so no
# window reaches a real screen.
smoke:
	scripts/ort-dylib.sh release
	npx tauri build --no-bundle
	@# Start Xvfb unless one is running (a socket alone may be stale).
	@if ! { [ -e /tmp/.X99-lock ] && kill -0 $$(tr -d ' ' < /tmp/.X99-lock) 2>/dev/null; }; then \
		rm -f /tmp/.X99-lock /tmp/.X11-unix/X99; \
		(Xvfb :99 -screen 0 1280x800x24 >/dev/null 2>&1 &); sleep 1; \
	fi
	env -u WAYLAND_DISPLAY DISPLAY=:99 GDK_BACKEND=x11 \
		node ui/smoke/app-smoke.mjs target/release/picturesque-desktop target/smoke-results

app-dev: ort-dylib
	npx tauri dev

# Installers (docs/development/packaging.rst). Each must be built on the OS
# it targets: the binary links the build host's libvips and LibRaw.
ifeq ($(OS),Windows_NT)
HOST_OS := Windows
else
HOST_OS := $(shell uname -s)
endif
require-os = @test "$(HOST_OS)" = "$(1)" || \
	(echo "$@ must run on $(1) (this is $(HOST_OS))" && exit 1)

dist:
ifeq ($(HOST_OS),Linux)
	$(MAKE) dist-linux
else ifeq ($(HOST_OS),Darwin)
	$(MAKE) dist-macos
else ifeq ($(HOST_OS),Windows)
	$(MAKE) dist-windows
else
	@echo "no installers for $(HOST_OS)" && exit 1
endif

dist-deb:
	$(call require-os,Linux)
	scripts/ort-dylib.sh release
	npx tauri build --bundles deb

dist-rpm:
	$(call require-os,Linux)
	scripts/ort-dylib.sh release
	npx tauri build --bundles rpm

# linuxdeploy runs extracted so no FUSE is needed.
dist-appimage:
	$(call require-os,Linux)
	scripts/ort-dylib.sh release
	APPIMAGE_EXTRACT_AND_RUN=1 npx tauri build --bundles appimage

dist-linux:
	$(call require-os,Linux)
	scripts/ort-dylib.sh release
	APPIMAGE_EXTRACT_AND_RUN=1 npx tauri build --bundles deb,rpm,appimage

dist-macos:
	$(call require-os,Darwin)
	npx tauri build --bundles app
	scripts/bundle-macos.sh

dist-windows:
	$(call require-os,Windows)
	npx tauri build --bundles msi,nsis

clean:
	cargo clean
	rm -rf $(DOCS_DIR)/_build

docs:
	$(SPHINX) sphinx-build -b html $(DOCS_DIR) $(DOCS_DIR)/_build/html

docs-serve:
	$(SPHINX) sphinx-autobuild --host 127.0.0.1 --port 8000 $(DOCS_DIR) $(DOCS_DIR)/_build/html

docs-check:
	$(SPHINX) sphinx-build -W --keep-going -n -b html $(DOCS_DIR) $(DOCS_DIR)/_build/html
	$(SPHINX) sphinx-build -W --keep-going -n -b linkcheck $(DOCS_DIR) $(DOCS_DIR)/_build/linkcheck

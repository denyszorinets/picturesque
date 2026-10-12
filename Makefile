.PHONY: help format format-check test lint build doc check clean ui-deps ui-check e2e bindings-check dev-web app-dev docs docs-serve docs-check

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
	@echo "  ui-check      Front-end type check, lint, format check and tests"
	@echo "  e2e           Browser end-to-end tests against the dev server (Playwright)"
	@echo "  bindings-check Fail if generated TypeScript API types are out of date"
	@echo "  dev-web       Run the UI in a browser against the dev server (no display needed)"
	@echo "  app-dev       Run the desktop app with hot reload (needs a display)"
	@echo "  clean         Remove build artifacts"
	@echo "  docs          Build documentation into docs/_build/html"
	@echo "  docs-serve    Live-reloading documentation preview on http://127.0.0.1:8000"
	@echo "  docs-check    Strict documentation build and link check"

format:
	cargo fmt --all
	npm run format -w ui

format-check:
	cargo fmt --all --check

test:
	cargo test --workspace --all-targets $(CARGO_FLAGS)
	cargo test --workspace --doc $(CARGO_FLAGS)

lint:
	cargo clippy --workspace --all-targets --all-features $(CARGO_FLAGS) -- -D warnings

build:
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

e2e:
	npm run e2e -w ui

# ts-rs regenerates ui/src/lib/api/generated when tests run.
bindings-check: test
	git diff --exit-code -- ui/src/lib/api/generated
	@test -z "$$(git ls-files --others --exclude-standard -- ui/src/lib/api/generated)" || \
		(echo "untracked generated bindings; commit them" && exit 1)

dev-web:
	cargo build -p picturesque-devserver
	trap 'kill 0' EXIT INT TERM; \
		cargo run --quiet -p picturesque-devserver & \
		npm run dev -w ui

app-dev:
	npx tauri dev

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

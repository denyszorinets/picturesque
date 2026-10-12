.PHONY: help format format-check test lint build doc check clean ui-deps ui-check app-dev docs docs-serve docs-check

DOCS_DIR := docs
SPHINX := uv run --project $(DOCS_DIR) --frozen

help:
	@echo "Available targets:"
	@echo "  format        Format Rust code"
	@echo "  format-check  Check Rust formatting"
	@echo "  test          Run Rust tests (unit, integration, doc)"
	@echo "  lint          Run clippy with warnings as errors"
	@echo "  build         Build all workspace targets"
	@echo "  doc           Build Rust API docs with warnings as errors"
	@echo "  check         format-check + lint + test + build + ui-check"
	@echo "  ui-deps       Install front-end dependencies (npm ci)"
	@echo "  ui-check      Front-end type check, lint, format check and tests"
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
	cargo test --workspace --all-targets
	cargo test --workspace --doc

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

build:
	cargo build --workspace --all-targets

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

check: format-check lint test build ui-check

ui-deps:
	npm ci

ui-check:
	npm run check -w ui
	npm run lint -w ui
	npm run test -w ui

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

.PHONY: help format format-check test lint build doc check clean docs docs-serve docs-check

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
	@echo "  check         format-check + lint + test + build"
	@echo "  clean         Remove build artifacts"
	@echo "  docs          Build documentation into docs/_build/html"
	@echo "  docs-serve    Live-reloading documentation preview on http://127.0.0.1:8000"
	@echo "  docs-check    Strict documentation build and link check"

format:
	cargo fmt --all

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

check: format-check lint test build

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

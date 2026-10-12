.PHONY: help docs docs-serve docs-check

DOCS_DIR := docs
SPHINX := uv run --project $(DOCS_DIR) --frozen

help:
	@echo "Available targets:"
	@echo "  docs          Build documentation into docs/_build/html"
	@echo "  docs-serve    Live-reloading documentation preview on http://127.0.0.1:8000"
	@echo "  docs-check    Strict documentation build and link check"

docs:
	$(SPHINX) sphinx-build -b html $(DOCS_DIR) $(DOCS_DIR)/_build/html

docs-serve:
	$(SPHINX) sphinx-autobuild --host 127.0.0.1 --port 8000 $(DOCS_DIR) $(DOCS_DIR)/_build/html

docs-check:
	$(SPHINX) sphinx-build -W --keep-going -n -b html $(DOCS_DIR) $(DOCS_DIR)/_build/html
	$(SPHINX) sphinx-build -W --keep-going -n -b linkcheck $(DOCS_DIR) $(DOCS_DIR)/_build/linkcheck

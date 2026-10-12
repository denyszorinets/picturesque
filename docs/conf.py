"""Sphinx configuration for the Picturesque documentation site."""

project = "Picturesque"
author = "Picturesque contributors"
copyright = "2026, Picturesque contributors"

REPO_URL = "https://github.com/denyszorinets/picturesque"

extensions = [
    "sphinx.ext.extlinks",
    "sphinx.ext.todo",
    "sphinx_design",
    "sphinxcontrib.mermaid",
]

extlinks = {
    "issue": (f"{REPO_URL}/issues/%s", "#%s"),
    "repo": (f"{REPO_URL}/blob/develop/%s", "%s"),
}

exclude_patterns = [
    "_build",
    ".venv",
    "superpowers",
    "SCAFFOLDING.md",
    "README.md",
]

nitpicky = True

html_theme = "furo"
html_title = "Picturesque"
html_theme_options = {
    "source_repository": REPO_URL,
    "source_branch": "develop",
    "source_directory": "docs/",
}

# The repository is private until it is published; anonymous link checks
# against it would return 404.
linkcheck_ignore = [rf"{REPO_URL}(/.*)?"]
linkcheck_timeout = 15

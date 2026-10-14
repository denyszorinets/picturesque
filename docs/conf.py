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
# Published under /docs/ of the website (docs/development/website.rst).
html_baseurl = "https://denyszorinets.github.io/picturesque/docs/"
html_theme_options = {
    "source_repository": REPO_URL,
    "source_branch": "develop",
    "source_directory": "docs/",
}

# Links into the repository are not checked: GitHub rate-limits anonymous
# link checkers, which would fail the build at random.
linkcheck_ignore = [
    rf"{REPO_URL}(/.*)?",
    # The website is deployed from this repository after the docs build.
    r"https://denyszorinets\.github\.io/picturesque(/.*)?",
    # Local development servers mentioned in the docs.
    r"https?://(127\.0\.0\.1|localhost)(:\d+)?(/.*)?",
]
linkcheck_timeout = 20
linkcheck_retries = 3

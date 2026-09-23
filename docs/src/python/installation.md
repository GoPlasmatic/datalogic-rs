# Installation

Install the pre-built `datalogic-py` package from PyPI:

```bash
# pip
pip install datalogic-py

# poetry
poetry add datalogic-py

# pipenv
pipenv install datalogic-py
```

## Supported Python Versions

`datalogic-py` supports **Python 3.10 and newer**. It builds with pyo3 against the PEP 384 Stable ABI (`abi3`), so:
* The same prebuilt wheel works across multiple minor Python versions (3.10, 3.11, 3.12, 3.13, etc.).
* Installing the wheel needs no local C compiler or Rust toolchain.

## Importing in Python

The distribution name and the import name differ:
* **PyPI Distribution name:** `datalogic-py` (with a hyphen)
* **Python import name:** `datalogic_py` (with an underscore, as Python import paths cannot contain hyphens)

```python
import datalogic_py
```

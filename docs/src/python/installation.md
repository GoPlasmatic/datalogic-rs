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
* One prebuilt wheel per platform works on every CPython release from 3.10 on; the package metadata lists 3.10 through 3.14.
* Installing the wheel needs no local C compiler or Rust toolchain.

PyPI carries wheels for Linux (manylinux and musllinux) on x86_64 and aarch64, macOS on x86_64 and arm64, and Windows on x86_64 and arm64. Every wheel ships type stubs and a `py.typed` marker, so mypy, pyright and IDE autocomplete see the whole API.

## Importing in Python

The distribution name and the import name differ:
* **PyPI Distribution name:** `datalogic-py` (with a hyphen)
* **Python import name:** `datalogic_py` (with an underscore, as Python import paths cannot contain hyphens)

```python
import datalogic_py

print(datalogic_py.__version__)  # "5.8.1"
```

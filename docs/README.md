# CRUDCrate Documentation

Interactive documentation for CRUDCrate.

## Local Development

```bash
# Install mdbook
cargo install mdbook

# Serve with live reload
mdbook serve --open
```

## Docker

```bash
# Build (from repo root, not docs folder)
cd /path/to/crudcrate
docker build -f docs/Dockerfile -t crudcrate-docs .

# Run
docker run -p 8080:80 crudcrate-docs

# Open http://localhost:8080
```

## Build Static Files

```bash
mdbook build
# Output in ./book/
```

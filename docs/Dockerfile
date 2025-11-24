# CRUDCrate Documentation
# Build from repo root in order to include changelog: 
#   docker build -f docs/Dockerfile -t crudcrate-docs .

# Stage 1: Build with mdBook
FROM rust:1.90-slim AS builder
RUN cargo install mdbook
WORKDIR /build

# Copy docs source and config (paths relative to build context = repo root)
COPY docs/src/ ./src/
COPY docs/book.toml ./book.toml
COPY docs/theme/ ./theme/

# Copy CHANGELOG from repo root (symlinks don't work in Docker)
COPY CHANGELOG.md ./src/changelog.md

# Build from /build where book.toml is located
RUN mdbook build

# Stage 2: Serve with nginx
FROM nginx:alpine
COPY --from=builder /build/book /usr/share/nginx/html
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]

# Diario-IA — tareas de desarrollo y build.
# Requiere `just` (https://github.com/casey/just). Ejecuta `just` para ver la lista.

set shell := ["bash", "-cu"]

default:
    @just --list

# --- Desarrollo ------------------------------------------------------------

# Servidor central en modo desarrollo (REST + SPA embebida en debug lee de disco).
dev:
    cargo run -p diario-server -- serve

# SPA con recarga en caliente (proxya /api/v1 al servidor central en :8787).
dev-client:
    cd crates/client && trunk serve --open

# Tests del servidor (storage + API + MCP).
test:
    cargo test -p diario-server

# Lint.
clippy:
    cargo clippy -p diario-server -- -D warnings

# --- Build de produccion ---------------------------------------------------

# Compila la SPA (WASM) a crates/client/dist.
build-client:
    cd crates/client && trunk build --release

# Binario nativo con la SPA embebida (un unico ejecutable).
build: build-client
    cargo build -p diario-server --release
    @echo "Binario: target/release/diario"

# Linux estatico (musl) — requiere cargo-zigbuild y zig.
#   cargo install cargo-zigbuild && brew install zig   (o pip install ziglang)
build-linux: build-client
    rustup target add x86_64-unknown-linux-musl
    cargo zigbuild -p diario-server --release --target x86_64-unknown-linux-musl
    @echo "Binario: target/x86_64-unknown-linux-musl/release/diario"

# Windows (.exe) desde cualquier host — requiere cargo-zigbuild y zig.
build-windows: build-client
    rustup target add x86_64-pc-windows-gnu
    cargo zigbuild -p diario-server --release --target x86_64-pc-windows-gnu
    @echo "Binario: target/x86_64-pc-windows-gnu/release/diario.exe"

# --- Utilidades ------------------------------------------------------------

# Crea una API key (muestra el token una sola vez).
key name:
    cargo run -p diario-server -- key create "{{name}}"

# Lista las API keys.
keys:
    cargo run -p diario-server -- key list

# Exporta el diario a ficheros markdown en ./export.
export:
    cargo run -p diario-server -- export

# Instala herramientas necesarias.
setup:
    rustup target add wasm32-unknown-unknown
    cargo install trunk --locked
    @echo "Para cross-compilar: cargo install cargo-zigbuild && brew install zig"

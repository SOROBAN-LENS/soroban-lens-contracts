# Convenience targets. Every target is a thin wrapper around cargo / stellar.
.PHONY: all test scenarios scenarios-json fmt fmt-check lint build check clean

all: check

## Run every unit test and the fixture verification suite.
test:
	cargo test --workspace

## Run the fixture catalog and print a human-readable report.
scenarios:
	cargo run -q -p scenario-runner

## Same, as JSON (for tooling / the backend).
scenarios-json:
	cargo run -q -p scenario-runner -- --json

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

## Build deployable Wasm. Requires stellar-cli >= 25.2.0 (soroban-sdk 29 refuses plain cargo Wasm builds).
build:
	stellar contract build

## What CI runs.
check: fmt-check lint test

clean:
	cargo clean

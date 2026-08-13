.PHONY: all test build fmt lint clean deploy-testnet

all: fmt lint test build

test:
	cargo test

build:
	cargo build --target wasm32v1-none --release

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets -- -D warnings

deploy-testnet:
	./scripts/deploy_testnet.sh

clean:
	cargo clean

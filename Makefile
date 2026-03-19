.PHONY: setup fmt check test

## Install git hooks (run once after clone)
setup:
	cp scripts/pre-commit .git/hooks/pre-commit
	chmod +x .git/hooks/pre-commit
	@echo "pre-commit hook installed"

## Format all code
fmt:
	cargo fmt --all

## Run all checks (same as CI)
check:
	cargo fmt --all -- --check
	cargo clippy --workspace -- -D warnings
	cargo test --workspace

## Run tests
test:
	cargo test --workspace

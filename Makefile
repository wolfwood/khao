.PHONY: clean
clean:
	rm -rf .cache
	cargo clean

.PHONY: run
run:
	cargo run

.PHONY: build
build:
	cargo build

.PHONY: fmt
fmt:
	cargo fmt

.PHONY: clean
clean:
	rm -f mycache.data
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

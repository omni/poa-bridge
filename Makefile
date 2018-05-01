all: target/release/bridge

.PHONY: target/release/bridge

target/release/bridge:
	cd cli && cargo build --release

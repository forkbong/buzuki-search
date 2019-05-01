build:
	cargo build -j2 --release

cross-build:
	cross build -j2 --target=armv7-unknown-linux-gnueabihf --release

install:
	cargo install -j2 --force --path .
	sudo ln -sf ${HOME}/.cargo/bin/buzuki-search /usr/local/bin/

sync:
	scp target/armv7-unknown-linux-gnueabihf/release/buzuki-search pi:/usr/local/bin/

.PHONY: build cross-build install sync

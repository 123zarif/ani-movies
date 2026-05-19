# Simple Makefile to build and install
PREFIX = /usr/local/bin

build:
	cargo build --release

install: build
	sudo cp target/release/ani-movies $(PREFIX)/ani-movies
	sudo chmod +x $(PREFIX)/ani-movies

uninstall:
	sudo rm $(PREFIX)/ani-movies

clean:
	cargo clean
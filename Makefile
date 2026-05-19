# Ani-Movies Makefile

IS_TERMUX := $(shell uname -o 2>/dev/null | grep -i android)

ifdef IS_TERMUX
    INSTALL_DIR = $(PREFIX)/bin
    CP_CMD = cp
    CHMOD_CMD = chmod
else
    INSTALL_DIR = /usr/local/bin
    CP_CMD = sudo cp
    CHMOD_CMD = sudo chmod
endif

build:
	cargo build --release

install: build
	@mkdir -p $(INSTALL_DIR)
	$(CP_CMD) target/release/ani-movies $(INSTALL_DIR)/ani-movies
	$(CHMOD_CMD) +x $(INSTALL_DIR)/ani-movies
	@echo "Installed to $(INSTALL_DIR)/ani-movies"

uninstall:
	@if [ -f $(INSTALL_DIR)/ani-movies ]; then \
		$(if $(IS_TERMUX), rm, sudo rm) $(INSTALL_DIR)/ani-movies; \
		echo "Uninstalled successfully."; \
	else \
		echo "File not found."; \
	fi

clean:
	cargo clean
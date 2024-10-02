# Variables
PACKAGE_NAME = website_monitor
BIN_DIR = /usr/local/bin
CONFIG_DIR = /etc/$(PACKAGE_NAME)
SERVICE_FILE = /etc/systemd/system/$(PACKAGE_NAME).service

# Default target
.PHONY: all
all: build

# Build the project in release mode
.PHONY: build
build:
	cargo build --release

# Install binary and configuration files
.PHONY: install
install: build
	@echo "Installing the $(PACKAGE_NAME) binary..."
	install -m 0755 target/release/$(PACKAGE_NAME) $(BIN_DIR)

	@echo "Installing configuration files..."
	install -d $(CONFIG_DIR)
	install -m 0644 Config.toml Overrides.toml $(CONFIG_DIR)

	@echo "Installing systemd service file..."
	install -m 0644 $(PACKAGE_NAME).service $(SERVICE_FILE)
	systemctl daemon-reload
	systemctl enable $(PACKAGE_NAME)
	systemctl start $(PACKAGE_NAME)

# Uninstall the binary, configuration files, and service
.PHONY: uninstall
uninstall:
	@echo "Stopping and removing systemd service..."
	systemctl stop $(PACKAGE_NAME)
	systemctl disable $(PACKAGE_NAME)
	rm -f $(SERVICE_FILE)
	systemctl daemon-reload

	@echo "Removing installed binary..."
	rm -f $(BIN_DIR)/$(PACKAGE_NAME)

	@echo "Removing configuration files..."
	rm -rf $(CONFIG_DIR)

# Clean build artifacts
.PHONY: clean
clean:
	cargo clean

# Run the application
.PHONY: run
run: build
	@echo "Running the application..."
	./target/release/$(PACKAGE_NAME)


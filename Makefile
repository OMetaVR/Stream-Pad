# StreamPad — build everything and install it for the current user.
#   make                 build daemon, overlay and editor (release)
#   make install         install to ~/.local (PREFIX=/usr/local for a system install: sudo make install)
#   make uninstall
#   make release         prebuilt tarball + checksums in dist/ (Arch package: packaging/arch/PKGBUILD)
#   make test            Rust unit tests (no devices touched)

PREFIX     ?= $(HOME)/.local
BUILD      := build
TARGET     := $(CURDIR)/target/release-all
JOBS       ?= $(shell nproc)

export CARGO_TARGET_DIR := $(TARGET)

.PHONY: all daemon overlay editor share install uninstall release test clean

all: daemon overlay editor share

daemon:
	cargo build --release -p streampadd -j $(JOBS)

overlay:
	cmake -S apps/overlay -B $(BUILD)/overlay -G Ninja -DCMAKE_BUILD_TYPE=Release
	cmake --build $(BUILD)/overlay -j $(JOBS)

editor:
	cd apps/editor && bun install --frozen-lockfile && bun tauri build --no-bundle

share:
	mkdir -p $(BUILD)/share/streampad/icons/lucide
	cp apps/editor/node_modules/lucide-static/icons/*.svg $(BUILD)/share/streampad/icons/lucide/
	cp apps/editor/node_modules/lucide-static/LICENSE $(BUILD)/share/streampad/icons/lucide/LICENSE 2>/dev/null || true

install:
	PREFIX="$(PREFIX)" TARGET="$(TARGET)" BUILD="$(BUILD)" ./packaging/install.sh

release: all
	./packaging/make-release.sh

uninstall:
	PREFIX="$(PREFIX)" ./packaging/uninstall.sh

test:
	cargo test -p streampad-core -p streampad-script -p streampadd -p streampad-input

clean:
	rm -rf $(BUILD) $(TARGET)

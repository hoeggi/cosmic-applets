# Host entry point. All Rust tooling runs inside the builder container;
# only `install`/`uninstall` (sudo) and `run`/`logs` touch the host.
#
#   make build                        # all applets, podman (default)
#   make run APPLET=headsetcontrol    # one applet in a standalone window
#   make deb                          # .deb packages in target/debian/
#   make build ENGINE=docker          # docker instead of podman

ifeq ($(shell id -u),0)
  $(error Run make as your normal user; it calls sudo itself where needed)
endif

ENGINE  ?= podman
IMAGE   := cosmic-applet-builder
STAMP   := .image.$(ENGINE).stamp
# Applets live in applets/<name>; the crate/binary/package is cosmic-applet-<name>.
APPLETS := $(notdir $(wildcard applets/*))
APPLET  ?= $(APPLETS)
PKGS    := $(addprefix cosmic-applet-,$(APPLET))
PKG_ARGS := $(addprefix -p ,$(PKGS))

ifeq ($(ENGINE),podman)
  USERFLAGS := --userns=keep-id
else
  USERFLAGS := --user $(shell id -u):$(shell id -g)
endif

# Allocate a TTY only when make itself runs in one.
TTY := $(shell [ -t 0 ] && echo -it)

RUN := $(ENGINE) run --rm $(TTY) $(USERFLAGS) \
	-v $(CURDIR):/src -w /src \
	-e HOME=/tmp -e USER=$(USER) -e CARGO_HOME=/src/.cache/cargo \
	$(IMAGE)

# The image is labelled with a hash of the Containerfile: an existing image
# (also one built from another checkout) is reused unless the Containerfile changed.
IMAGE_HASH := $(shell sha256sum container/Containerfile | cut -c1-16)

.PHONY: image build check test fmt run deb install uninstall shell clean reload-panel logs

$(STAMP): container/Containerfile
	@if [ "$$($(ENGINE) image inspect --format '{{index .Config.Labels "containerfile.hash"}}' $(IMAGE) 2>/dev/null)" != "$(IMAGE_HASH)" ]; then \
		$(ENGINE) build --label containerfile.hash=$(IMAGE_HASH) -f container/Containerfile -t $(IMAGE) container/; \
	fi
	touch $@

image:
	$(ENGINE) build --label containerfile.hash=$(IMAGE_HASH) -f container/Containerfile -t $(IMAGE) container/
	touch $(STAMP)

build: $(STAMP)
	$(RUN) cargo build --release $(PKG_ARGS)

check: $(STAMP)
	$(RUN) sh -ec 'cargo fmt --all --check && cargo clippy --release --workspace --all-targets -- -D warnings'

test: $(STAMP)
	$(RUN) cargo test --release --workspace

fmt: $(STAMP)
	$(RUN) cargo fmt --all

run: build
	@[ $(words $(APPLET)) -eq 1 ] || { echo "usage: make run APPLET=<$(subst $() ,|,$(APPLETS))>"; exit 1; }
	RUST_LOG=$${RUST_LOG:-info} ./target/release/cosmic-applet-$(APPLET)

# One .deb per applet in target/debian/ (same packaging as the GitHub release workflow).
deb: build
	$(RUN) sh -ec 'for p in $(PKGS); do cargo deb --no-build -p $$p; done'

# Installs through apt, so files are tracked and `make uninstall` removes them cleanly.
install: deb
	sudo apt-get install -y --reinstall $$(for p in $(PKGS); do ls -t ./target/debian/$${p}_*.deb | head -n1; done)

uninstall:
	sudo apt-get remove -y $(PKGS)

shell: $(STAMP)
	$(RUN) bash

clean:
	rm -rf target applets/*/generated

reload-panel:
	killall cosmic-panel

# cosmic-panel relays applet stderr under its own identifier, prefixed with the app ID.
logs:
	journalctl --user -b -f -t cosmic-panel | grep --line-buffered 'at.hoeggi.CosmicApplet'

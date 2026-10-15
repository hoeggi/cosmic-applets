# Host entry point. All Rust tooling runs inside the builder container;
# only `install`/`uninstall` (sudo) and `run`/`logs` touch the host.
#
#   make build                 # podman (default)
#   make build ENGINE=docker   # docker

ifeq ($(shell id -u),0)
  $(error Run make as your normal user; it calls sudo itself where needed)
endif

ENGINE  ?= podman
IMAGE   := cosmic-applet-builder
NAME    := cosmic-applet-commands
APPID   := at.hoeggi.CosmicAppletCommands
PREFIX  := /usr
STAGE   := target/stage
STAMP   := .image.$(ENGINE).stamp

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

.PHONY: image generate build check fmt run stage install uninstall shell clean reload-panel logs

$(STAMP): container/Containerfile
	$(ENGINE) build -f container/Containerfile -t $(IMAGE) container/
	touch $@

image:
	rm -f $(STAMP)
	$(MAKE) $(STAMP)

# One-time: render pop-os/cosmic-applet-template into the repo root.
generate: $(STAMP)
	$(RUN) sh -ec '\
		cd /tmp && cargo generate --git https://github.com/pop-os/cosmic-applet-template \
			--name $(NAME) --silent \
			-d appid=$(APPID) \
			-d "description=Run configurable commands from the COSMIC panel" \
			-d license=MPL-2.0 \
			-d repository-url=https://github.com/hoeggi/$(NAME) && \
		cp -r --update=none /tmp/$(NAME)/. /src/'

build: $(STAMP)
	$(RUN) just build-release

check: $(STAMP)
	$(RUN) sh -ec 'cargo fmt --check && cargo clippy --release'

fmt: $(STAMP)
	$(RUN) cargo fmt

run: build
	RUST_LOG=$${RUST_LOG:-info} ./target/release/$(NAME)

stage: build
	rm -rf $(STAGE)
	$(RUN) just rootdir=/src/$(STAGE) prefix=$(PREFIX) install

# --remove-destination replaces the binary even while the panel is running it.
install: stage
	sudo cp -r --remove-destination $(STAGE)$(PREFIX)/. $(PREFIX)/

uninstall:
	sudo rm -f $(PREFIX)/bin/$(NAME) \
		$(PREFIX)/share/applications/$(APPID).desktop \
		$(PREFIX)/share/appdata/$(APPID).metainfo.xml \
		$(PREFIX)/share/icons/hicolor/scalable/apps/$(APPID).svg

shell: $(STAMP)
	$(RUN) bash

clean:
	rm -rf target

reload-panel:
	killall cosmic-panel

# cosmic-panel relays applet stderr under its own identifier, prefixed with the app ID.
logs:
	journalctl --user -b -f -t cosmic-panel | grep --line-buffered '$(APPID):'

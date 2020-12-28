.DEFAULT_GOAL := default

THRIFT_OUT := src/generated
THRIFT_SRC := idls/meta.thrift
THRIFT_BIN := thrift

default: clean build

clean:
	cargo clean;
	# delete generated sources without mod.rs
	find src/base | grep .rs | grep -v mod.rs | xargs rm;

submodules:
	git submodule update --init --recursive;

codegen: submodules
	mkdir -p $(THRIFT_OUT);
	$(foreach SOURCE,$(THRIFT_SRC),\
		$(THRIFT_BIN) \
			--gen rs \
			--out $(THRIFT_OUT) \
			$(SOURCE);)

build: codegen
	cargo build;

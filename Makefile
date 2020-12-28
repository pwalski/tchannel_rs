.DEFAULT_GOAL := default

CORE := tchannel-core
THRIFT_OUT := $(CORE)/src/gen
THRIFT_SRC := idls/idl/github.com/uber/tchannel/meta.thrift
THRIFT_BIN := thrift

default: clean build

clean:
	cargo clean;
	# delete generated sources without mod.rs
	find $(CORE)/src/gen | grep .rs | xargs rm;

submodules:
	git submodule update --init;

codegen: submodules
	mkdir -p $(THRIFT_OUT);
	$(foreach SOURCE,$(THRIFT_SRC),\
		$(THRIFT_BIN) \
			--gen rs \
			--out $(THRIFT_OUT) \
			$(SOURCE);)

build: codegen
	cargo build;

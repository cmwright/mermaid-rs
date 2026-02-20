FIXTURES_DIR := tests/integration/fixtures
OUTPUT_DIR := /tmp/mermaid-test-svgs
COMPLEX_INPUT := /tmp/complex-test.mmd
BINARY := cargo run --

FIXTURES := $(wildcard $(FIXTURES_DIR)/*.mmd)
FIXTURE_SVGS := $(patsubst $(FIXTURES_DIR)/%.mmd,$(OUTPUT_DIR)/%.svg,$(FIXTURES))

.PHONY: test-svgs clean-svgs build test test-examples build-wasm examples serve-examples

build:
	cargo build

test:
	cargo test

test-svgs: build $(FIXTURE_SVGS)
	@if [ -f "$(COMPLEX_INPUT)" ]; then \
		$(BINARY) --input $(COMPLEX_INPUT) --output $(OUTPUT_DIR)/complex.svg; \
		echo "Rendered $(COMPLEX_INPUT) -> $(OUTPUT_DIR)/complex.svg"; \
	fi
	@echo "All SVGs written to $(OUTPUT_DIR)/"

$(OUTPUT_DIR)/%.svg: $(FIXTURES_DIR)/%.mmd | $(OUTPUT_DIR)
	$(BINARY) --input $< --output $@
	@echo "Rendered $< -> $@"

$(OUTPUT_DIR):
	mkdir -p $(OUTPUT_DIR)

test-examples:
	cargo test -p mermaid-rs --test examples_comparison -- --nocapture
	@echo "Open target/examples-comparison.html in your browser"

build-wasm:
	@command -v wasm-pack >/dev/null 2>&1 || { echo "Install wasm-pack: cargo install wasm-pack"; exit 1; }
	wasm-pack build crates/mermaid-wasm --target web --out-dir ../../target/wasm-pkg

examples: build-wasm
	cargo test -p mermaid-rs --test examples_comparison -- --nocapture
	@echo "Open target/examples-comparison.html in your browser"

serve-examples: examples
	python3 -m http.server 8080 --directory target

clean-svgs:
	rm -rf $(OUTPUT_DIR)

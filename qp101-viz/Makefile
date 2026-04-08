TYPST ?= typst
ROOT ?= .
EXAMPLE_DIR ?= examples
OUT_DIR ?= draft_figs

EXAMPLE_SRCS := $(wildcard $(EXAMPLE_DIR)/*.typ)
PDFS := $(patsubst $(EXAMPLE_DIR)/%.typ,$(OUT_DIR)/%.pdf,$(EXAMPLE_SRCS))

.PHONY: all list clean

all: $(PDFS)

list:
	@printf '%s\n' $(PDFS)

$(OUT_DIR):
	@mkdir -p $(OUT_DIR)

$(OUT_DIR)/%.pdf: $(EXAMPLE_DIR)/%.typ lib.typ typst.toml | $(OUT_DIR)
	$(TYPST) compile --root $(ROOT) $< $@

clean:
	rm -f $(PDFS)

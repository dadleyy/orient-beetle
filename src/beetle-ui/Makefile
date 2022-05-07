ELM=elm
NPM=npm

DEBUG_DIR=./target/debug
MAIN_JS=./target/debug/main.js
BOOT_JS=./target/debug/boot.js
INDEX_HTML=./target/debug/index.html

RELEASE_DIR=./target/release
RELEASE_MAIN_JS=$(subst debug,release,$(MAIN_JS))
RELEASE_BOOT_JS=$(subst .js,.min.js,$(subst debug,release,$(BOOT_JS)))
RELEASE_INDEX=$(subst debug,release,$(INDEX_HTML))

.PHONY: all clean

all: $(MAIN_JS) $(BOOT_JS) $(INDEX_HTML)

clean:
	rm -rf $(DEBUG_DIR)
	rm -rf $(RELEASE_DIR)

release: $(RELEASE_MAIN_JS) $(RELEASE_BOOT_JS) $(RELEASE_INDEX)

$(RELEASE_MAIN_JS): ./src/Main.elm
	$(ELM) make --optimize --output $@ $^
	$(NPM) run min -- $@ --output $(subst .js,.min.js,$@)

$(RELEASE_BOOT_JS): $(BOOT_JS)
	$(NPM) run min -- $^ --output $@

$(RELEASE_INDEX): ./src/index.pug
	$(NPM) run pug:release -- -o $(RELEASE_DIR) $^

$(MAIN_JS): ./src/Main.elm
	$(ELM) make $^ --output $(MAIN_JS)

$(BOOT_JS): ./src/boot.ts @types/global.d.ts
	$(NPM) run tsc -- --outDir ./target/debug

$(INDEX_HTML): ./src/index.pug
	$(NPM) run pug -- -o $(DEBUG_DIR) $^

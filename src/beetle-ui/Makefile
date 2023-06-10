ELM=elm
NPM=npm

ELM_SOURCES=$(wildcard src/*.elm) $(wildcard src/*/*.elm)
DEBUG_DIR=./target/debug
MAIN_JS=./target/debug/main.js
BOOT_JS=./target/debug/boot.js
INDEX_HTML=./target/debug/index.html
MAIN_CSS=./target/debug/main.css

RELEASE_DIR=./target/release
RELEASE_MAIN_JS=$(subst debug,release,$(MAIN_JS))
RELEASE_MAIN_CSS=$(subst debug,release,$(MAIN_CSS))
RELEASE_BOOT_JS=$(subst .js,.min.js,$(subst debug,release,$(BOOT_JS)))
RELEASE_INDEX=$(subst debug,release,$(INDEX_HTML))

DEBUG_FAVICON=$(subst ./static,target/debug,$(wildcard ./static/favicon/*.png))

.PHONY: all clean fmt debug_font release_font debug_favicon release_favicon bundle

bundle: fmt all

all: $(MAIN_JS) $(BOOT_JS) $(INDEX_HTML) $(MAIN_CSS) debug_font debug_favicon

release: $(RELEASE_MAIN_JS) $(RELEASE_BOOT_JS) $(RELEASE_INDEX) $(RELEASE_MAIN_CSS) release_font release_favicon

fmt:
	$(NPM) run lint:elm -- --yes
	$(NPM) run lint:ts -- --fix

clean:
	rm -rf $(DEBUG_DIR)
	rm -rf $(RELEASE_DIR)

##
# HTML
##

$(INDEX_HTML): ./src/index.pug
	$(NPM) run pug -- -o $(DEBUG_DIR) $<

$(RELEASE_INDEX): ./src/index.pug $(INDEX_HTML)
	$(NPM) run pug:release -- -o $(RELEASE_DIR) $<

##
# JS
##

$(MAIN_JS): ./src/Main.elm $(ELM_SOURCES)
	$(ELM) make $< --output $(MAIN_JS)

$(BOOT_JS): ./src/boot.ts @types/global.d.ts
	$(NPM) run tsc -- --outDir ./target/debug

$(RELEASE_MAIN_JS): ./src/Main.elm $(MAIN_JS)
	$(ELM) make --optimize --output $@ $<
	$(NPM) run min -- $@ --output $(subst .js,.min.js,$@) --mangle

$(RELEASE_BOOT_JS): $(BOOT_JS)
	$(NPM) run min -- $< --output $@ --mangle

##
# CSS
##

$(MAIN_CSS): ./src/main.css $(ELM_SOURCES) ./src/index.pug src/boot.ts
	$(NPM) run postcss -- $< --output $@

# relies on sass compiling vanilla `.css` correctly.
$(RELEASE_MAIN_CSS): ./src/main.css $(ELM_SOURCES)
	$(NPM) run postcss -- $< --output $@

release_favicon: \
	target/release/favicon/favicon.ico \
	target/release/favicon/favicon-32x32.png \
	target/release/favicon/favicon-16x16.png \
	target/release/favicon/apple-touch-icon.png \
	target/release/favicon/android-chrome-512x512.png \
	target/release/favicon/android-chrome-192x192.png

target/debug/favicon/%: static/favicon/%
	@mkdir -p target/debug/favicon
	cp -v $< $@

debug_favicon: \
	target/debug/favicon/favicon.ico \
	target/debug/favicon/favicon-32x32.png \
	target/debug/favicon/favicon-16x16.png \
	target/debug/favicon/apple-touch-icon.png \
	target/debug/favicon/android-chrome-512x512.png \
	target/debug/favicon/android-chrome-192x192.png

target/release/favicon/%: static/favicon/%
	@mkdir -p target/release/favicon
	cp -v $< $@

debug_font: \
	target/debug/icon-font/style.css \
	target/debug/icon-font/fonts/icomoon.ttf \
	target/debug/icon-font/fonts/icomoon.eot \
	target/debug/icon-font/fonts/icomoon.svg \
	target/debug/icon-font/fonts/icomoon.woff

target/debug/icon-font/%.css: static/icon-font/%.css
	mkdir -p target/debug/icon-font
	cp -v $< $@

target/debug/icon-font/fonts/%: static/icon-font/fonts/%
	mkdir -p target/debug/icon-font/fonts
	cp -v $< $@

release_font: \
	target/release/icon-font/style.css \
	target/release/icon-font/fonts/icomoon.ttf \
	target/release/icon-font/fonts/icomoon.eot \
	target/release/icon-font/fonts/icomoon.svg \
	target/release/icon-font/fonts/icomoon.woff

target/release/icon-font/%.css: static/icon-font/%.css
	mkdir -p target/release/icon-font
	cp -v $< $@

target/release/icon-font/fonts/%: static/icon-font/fonts/%
	mkdir -p target/release/icon-font/fonts
	cp -v $< $@

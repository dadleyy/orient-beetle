PYTHON=python3

ESPTOOL=$(HOME)/.platformio/packages/tool-esptoolpy/esptool.py
BOOT_APP0=$(HOME)/.platformio/packages/framework-arduinoespressif32/tools/partitions/boot_app0.bin

# TODO: consult either esptool.py or platformio. they handle this gracefully.
PORT=/dev/cu.usbmodem101

BINS=.pio/build/xiao/bootloader.bin .pio/build/xiao/partitions.bin .pio/build/xiao/firmware.bin
MERGED_BIN=beetle-merged-flash.bin

.PHONY: clean flash-merged tool

all: $(MERGED_BIN)

clean:
	rm $(BINS)

$(BINS):
	pio run

$(MERGED_BIN): $(BINS)
	$(PYTHON) $(ESPTOOL) --chip esp32c3 merge_bin -o $@ \
		--flash_size 4MB \
		0x0000 .pio/build/xiao/bootloader.bin \
		0x8000 .pio/build/xiao/partitions.bin \
		0xe000 $(BOOT_APP0) \
		0x10000 .pio/build/xiao/firmware.bin

help:
	$(PYTHON) $(ESPTOOL) --help

flash-merged: $(MERGED_BIN)
	echo $<
	$(PYTHON) $(ESPTOOL) --chip esp32c3 --port $(PORT) write_flash --flash_mode dio --flash_freq 80m 0x0 $<

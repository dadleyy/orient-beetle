# Orient-Beetle Firmware

This is the [platformio]-based project with the source code for the firmware
that runs on the [firebeetle] microcontroller board.


## Generating Fonts

The fonts used by the firmware have been "compiled" down from `.ttf` files
into gfx-specific header files using the [fontgen] tool provided from the
main gfx repo. Assuming you have compiled the `fontgen.cpp` file and have
the executable somewhere in your `$PATH`, you may use the `gen_fonts.sh`
shell script in this directory to generate fresh `include/<font>.hpp` files.

A mapping of characters to icons can be found in the [`mapping.md`][mm] file
located in the `.resources/Glyphter-font` directory at the root of this repo.
 
## Helpful Links

1. [ESP32 Arduino Core][esp32-arduino-core]
2. [`lib/README`](./lib/README.md)
2. [`include/README`](./include/README.md)


[← README](../../README.md)

[platformio]: https://platformio.org
[firebeetle]: https://www.dfrobot.com/product-2195.html
[esp32-arduino-core]: https://github.com/espressif/arduino-esp32/tree/master/libraries
[fontgen]: https://github.com/codewitch-honey-crisis/gfx/blob/master/tools/fontgen.cpp
[mm]: ../../.resources/Glyphter-font/mapping.md

# Orient-Beetle Firmware

This is the [platformio]-based project with the source code for the firmware
that runs on the [firebeetle] microcontroller board.

## Generating Fonts

The fonts used by the firmware have been "compiled" down from `.ttf` files
into lvgl-specific header files using the [fontconverter] tool provided by
the maintainers of that framework.

A mapping of characters to icons can be found in the [`mapping.md`][mm] file
located in the `.resources/Glyphter-font` directory at the root of this repo.
 
## Helpful Links

1. [ESP32 Arduino Core][esp32-arduino-core]
1. [lvgl][lvgl]
2. [`lib/README`](./lib/README.md)
2. [`include/README`](./include/README.md)

[← README](../../README.md)


[platformio]: https://platformio.org
[firebeetle]: https://www.dfrobot.com/product-2195.html
[esp32-arduino-core]: https://github.com/espressif/arduino-esp32/tree/master/libraries
[mm]: ../../.resources/Glyphter-font/mapping.md
[fontconverter]: https://lvgl.io/tools/fontconverter
[lvgl]: https://docs.lvgl.io/8/

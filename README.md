## Orient-Beetle

A project that incorporates a wifi-enabled microcontroller, a tft/lcd display and
a proximity sensor.


### Building: Firmware

The firmware for the application lives in `src/beetle-pio` and can be compiled
using the [platformIO cli][pio].

```
$ cd src/beetle-pio
$ pio run -t upload             <- will attempt to compile + upload to device
$ pio run -t upload -e release  <- builds without Serial logs
```

### Hardware

For a list of harware involved, see [`.docs/hardware.md`](/.docs/hardware.md).

[pio]: https://docs.platformio.org/en/stable/core/index.html

### WiFi Configuration HTML Generator

This tool is used to create the [`index_html.hpp`][index] file will be sent to new
http connections during the wifi manager's configuration mode.

```
$ cargo build --release
$ ./target/release/wchgen \
  ../../src/beetle-pio/include/index.html > ../../src/beetle-pio/include/index_html.hpp
```
 
[← README](../../README.md)

[index]: ../../src/beetle-pio/include/index.html

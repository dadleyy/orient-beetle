## Orient-Beetle

A project that incorporates a wifi-enabled microcontroller, a tft/lcd display and
a proximity sensor.

### Building: Firmware

The firmware for the application lives in `src/beetle-pio` and can be compiled
using the [platformIO cli][pio]. Before compiling, please make sure to set the
following environment variables, which are expected to be defined at compile time:


```
REDIS_PORT=""
REDIS_HOST=""
REDIS_AUTH=""
```

**Tip**: It is helpful to define these in a `.env` file within the `src/beetle-io`
directory and source them automatically using a tool like [this zsh plugin][dotenv].

In addition, you will need to download the root ca certificate for your redis host
and save it to:

```
src/beetle-pio/certs/redis_host_root_ca.pem
```

The contents of this file are loaded into flash memory via the
`board_build.embed_txtfiles` setting defined in the project's `platform.ini`
file.

_For more information on how to prepare the ssl/tls components for our redis
connection, refer to [`.docs/redis-help.md`](.docs/redis-help.md)_.

Once your environment and certificate file have been prepared, the firmware can
be compiled from the `beetle-pio` directory:

```
$ cd src/beetle-pio
$ pio run -t upload             <- will attempt to compile + upload to device
$ pio run -t upload -e release  <- builds without Serial logs
```

### Hardware & Documentation

For a list of harware involved and other documentation, see [`.docs/README.md`](/.docs/README.md).

### Tools

1. [WiFi Configuration HTML Generator][wchgen] - This tiny rust application is used to generate the
contents of the `src/beetle-pio/include/index_html.hpp` file from and `index.html` file input.

[pio]: https://docs.platformio.org/en/stable/core/index.html
[dotenv]: https://github.com/ohmyzsh/ohmyzsh/blob/master/plugins/dotenv/dotenv.plugin.zsh
[wchgen]: ./tools/wchgen/README.md

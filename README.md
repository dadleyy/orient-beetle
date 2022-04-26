# Orient-Beetle

A project that incorporates a wifi-enabled microcontroller, a tft/lcd display and
a proximity sensor.

## I. Building: Firmware

The firmware for the application lives in `src/beetle-pio` and can be compiled
using the [platformIO cli][pio]. 


### I.I Environment Setup: Redis Environment Variables

The firmware relies on compile-time configuration information to determine what server,
port, and authentication information to use when establishing it's secure connection to
your redis instance. The most straightforward way to set this up is to define a `.env`
file at the root of the `beetle-io` platformio application:

```
$ cat ./src/beetle-io/.env

REDIS_PORT=1234
REDIS_HOST="my-redis.my-host.com"
REDIS_AUTH="1bc2ad"
```

This file is read by the [`extra_scripts`][extra_scripts] entry [`load_env.py`][lenv] and will
automatically attempt to set the correct `-DREDIS_HOST`, `-DREDIS_PORT`, and `-DREDIS_AUTH`
compiler flags.

### I.II Environment Setup: Redis Host CA Certificate

In order for the esp32 module to connect over tls to your redis host, you will need
to download the root ca certificate for the host and save it to:

```
src/beetle-pio/certs/redis_host_root_ca.pem
```

The contents of this file are loaded into flash memory via the
`board_build.embed_txtfiles` setting defined in the project's `platform.ini`
file.

_For more information on how to prepare the ssl/tls components for our redis
connection, refer to [`.docs/redis-help.md`](.docs/redis-help.md)_.

### I.III Compiling With `pio`

Once your environment and certificate file have been prepared, the firmware can
be compiled from the `beetle-pio` directory:

```
$ cd src/beetle-pio
$ pio run -t upload             <- will attempt to compile + upload to device
$ pio run -t upload -e release  <- builds without Serial logs
```

----

## II Hardware & Documentation

For a list of harware involved and other documentation, see [`.docs/README.md`](/.docs/README.md).

## III Miscellaneous Tools

1. [WiFi Configuration HTML Generator][wchgen] - This tiny rust application is used to generate the
contents of the `src/beetle-pio/include/index_html.hpp` file from and `index.html` file input.

[pio]: https://docs.platformio.org/en/stable/core/index.html
[dotenv]: https://github.com/ohmyzsh/ohmyzsh/blob/master/plugins/dotenv/dotenv.plugin.zsh
[wchgen]: ./tools/wchgen/README.md
[extra_scripts]: https://docs.platformio.org/en/latest/scripting/actions.html
[lenv]: ./src/beetle-io/load_env.py

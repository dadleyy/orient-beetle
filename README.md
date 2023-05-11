# Orient-Beetle

A project that incorporates a wifi-enabled microcontroller, a tft/lcd display and
a proximity sensor.

For general platform architecture documentation, see [`.docs/architecture.md`][arch].

For the hardware bill of materials, see [`.docs/hardware.md`][bom]

## Beetle PIO (Firmware)

The firmware for the application lives in [`src/beetle-pio`][fm-rd] and can be compiled
using the [platformIO cli][pio]. 

### Environment Setup: Redis Environment Variables

The firmware relies on compile-time configuration information to determine what server,
port, and authentication information to use when establishing it's secure connection to
your redis instance. The most straightforward way to set this up is to define a `.env`
file at the root of the `beetle-io` platformio application:

```
$ cat ./src/beetle-io/.env

REDIS_PORT=1234
REDIS_HOST="my-redis.my-host.com"
REDIS_AUTH_USERNAME="orient-beetle-device-id-consumer"
REDIS_AUTH_PASSWORD="1bc2ad"
```

This file is read by the [`extra_scripts`][extra_scripts] entry [`load_env.py`][lenv] and will
automatically attempt to set the correct `-DREDIS_HOST`, `-DREDIS_PORT`, and auth compiler flags.

The values of the `REDIS_AUTH_USERNAME` and `REDIS_AUTH_PASSWORD` values
require successfully "provisioning" your environment (more on this in the
[`srv` documentation](#beetle-srv-(web-backend)) below).

### Environment Setup: Redis Host CA Certificate

In order for the esp32 module to connect over tls to your redis host, you will need
to download the root ca certificate for the host and save it to:

```
src/beetle-pio/embeds/redis_host_root_ca.pem
```

The contents of this file are loaded into flash memory via the
`board_build.embed_txtfiles` setting defined in the project's `platform.ini`
file.

_For more information on how to prepare the ssl/tls components for our redis
connection, refer to [`.docs/redis-help.md`](.docs/redis-help.md)_.

### Compiling With `pio`

Once your environment and certificate file have been prepared, the firmware can
be compiled from the `beetle-pio` directory:

```
$ cd src/beetle-pio
$ pio run -t upload             <- will attempt to compile + upload to device
$ pio run -t upload -e release  <- builds without Serial logs
```

### Hardware & Documentation

For a list of harware involved and other documentation, see [`.docs/README.md`](/.docs/README.md).

#### Troubleshooting with `src/beetle-pio-tls-tester`

It can be ticky to know which certificate is required for a valid connection over tls with
your hosted redis provider. To help validate your certificate, the
[`src/beetle-pio-tls-tester`](./src/beetle-pio-tls-tester) directory contains a platformio
project can can be flashed onto your ESP32 to verify.

This project uses a very similar to configuration to the main `beetle-pio` project; you are
expected to provide both a `.env` file and `embeds/redis_host_root_ca.pem` file. If your
certificate file is valid, you should see something like:

```
[  4353][V][ssl_client.cpp:59] start_ssl_client(): Free internal heap before TLS 225035
[  4361][V][ssl_client.cpp:65] start_ssl_client(): Starting socket
[  4707][V][ssl_client.cpp:141] start_ssl_client(): Seeding the random number generator
[  4709][V][ssl_client.cpp:150] start_ssl_client(): Setting up the SSL/TLS structure...
[  4712][V][ssl_client.cpp:166] start_ssl_client(): Loading CA cert
[  4782][V][ssl_client.cpp:234] start_ssl_client(): Setting hostname for TLS session...
[  4783][V][ssl_client.cpp:249] start_ssl_client(): Performing the SSL/TLS handshake...
[  6214][V][ssl_client.cpp:270] start_ssl_client(): Verifying peer X.509 certificate...
[  6215][V][ssl_client.cpp:279] start_ssl_client(): Certificate verified.
[  6218][V][ssl_client.cpp:294] start_ssl_client(): Free internal heap after TLS 177751
[  6226][D][main.cpp:26] loop(): connection result: 1
[  7231][D][main.cpp:18] loop(): connected: 1
```

This assumes you have compiled and uploaded via `pio`:

```
$ pio run -t upload
$ screen /dev/ttyUSB0 115200
```

----

## Beetle SRV (Web Backend)

There are several rust exectuables that live in the [`srv/beetle-srv`](./src/beetle-srv/README.md)
directory. This includes the http api, a dirty little cli for troubleshooting, and a background worker
responsible for applying logic based on device connections. These applications generally require
an `env.toml` file with the various secrets that will be used for redis, mongo, etc... An example 
file can be seen at [`src/beetle-src/env.example.toml`](src/beetle-srv/env.example.toml)


### Creating Redis ACL for all devices

As part of provisioning your environment, you will need to create the appropriate redis acl entry that
will be flashed onto all devices and should only be able to `pop` ids off our registrar index:

```
$ cd src/beetle-srv
$ cargo run cli provision orient-beetle-id-consumer abc1234
```

----

## Beetle UI (Web Frontend)

The web ui for this project can be found in the [`src/beetle-ui`](./src/beetle-ui/README.md) directory.

----

## Miscellaneous Tools

1. [WiFi Configuration HTML Generator][wchgen] - This tiny rust application is used to generate the
contents of the `src/beetle-pio/embed/index.html` file from an `index.html` file input.

--- 

## Nice Third Party Providers for Services Used At Runtime

- redis @ [upstash](https://upstash.com/) - supports tls _and_ `ACL` command management. not really free.
- mongo @ [cloud.mongodb.com](https://cloud.mongodb.com) - has a free tier for hosted mongodb databases/clusters.
- oauth @ [auth0](https://manage.auth0.com) - has a free tier for managing your login page.

---

This project was named by combining the name of the development board (fire<i>beetle</i>) with the name of
the manufacturer of the display being used (_orient_ displays).

[pio]: https://docs.platformio.org/en/stable/core/index.html
[dotenv]: https://github.com/ohmyzsh/ohmyzsh/blob/master/plugins/dotenv/dotenv.plugin.zsh
[wchgen]: ./tools/wchgen/README.md
[extra_scripts]: https://docs.platformio.org/en/latest/scripting/actions.html
[lenv]: ./src/beetle-pio/load_env.py
[fm-rd]: ./src/beetle-pio/README.md
[arch]: ./.docs/architecture.md
[bom]: ./.docs/hardware.md

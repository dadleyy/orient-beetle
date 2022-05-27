## Beetle UI

This directory contains the beetle web (html/js) application written in [`elm`]. In addition to elm,
the ui requires a few node/npm packages to help facilitate the generation of the main `index.html`
file, as well as minify/uglify the compiled `js` files from elm source code.

> Note: one of the goals of this project is to keep the js/npm/node footprint as minimal as possible,
> but [`pug`] and [`tsc`] are extremely helpful for generting the "peripheral" dependencies of the elm
> artifact.

### Building

This project leverages [`make`] to compile both the elm source code, in addition to any compilation
handled by node/npm packages:

```
$ make          <- compiles into ./target/debug
$ make release  <- compiles into ./target/release
```

Once compiled, the application can be "run" using any http server capable of serving static files,
or using the `dev-server.js` included in the root of this repo:

```
$ npm run start:dev-server
```

> Note: By default, this will attempt to proxy any `/api` requests to a running intance of the 
> [`beetle-web`](../beetle-srv/README.md) application; be sure to have that ready.


### Environment

There are a few environment-specific settings that can be changed:

```
BEETLE_UI_ROOT   -> Used to control where static assets are referenced by the `index.pug`/`html`
                    file.  Locally this defaults to `/`, but in production this allows a hosting
                    the application behind a path on an existing hostname.

BEETLE_API_ROOT  -> Specifies where api requests should be sent. Locally this defaults to `/api`
                    which is specifically handled by the `dev-server.js` file.

BEETLE_LOGIN_URL -> The url that the UI will send a user to in order to start the oauth redirect
                    + token flow.

```

To get an idea how this fits into the production environment, see the
`.github/workflows/build-and-publish.yml` workflow at the root of this repository.

[← README](../../README.md)

[`elm`]: https://elm-lang.org/
[`pug`]: https://pugjs.org/api/getting-started.html
[`tsc`]: https://www.typescriptlang.org/
[`make`]: https://www.gnu.org/software/make/

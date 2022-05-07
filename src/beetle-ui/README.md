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
or using the `http-server` included in the npm module dependencies:

```
$ npm run dev:server
```

> Note: By default, this will attempt to proxy any "unrecognized" requests to a running intance of the 
> [`beetle-web`](../beetle-srv/README.md) application; be sure to have that ready.
>
> TODO: Currently, this also means that true single-page application functionality will be broken; when
> refreshing the browser following an in-app link transition, the server will proxy to the backend,
> rather than serve the `index.html`.

[← README](../../README.md)

[`elm`]: https://elm-lang.org/
[`pug`]: https://pugjs.org/api/getting-started.html
[`tsc`]: https://www.typescriptlang.org/
[`make`]: https://www.gnu.org/software/make/

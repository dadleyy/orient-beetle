const http = require('http');
const url = require('url');
const path = require('path');
const fs = require('fs');
const httpProxy = require('http-proxy');
const dotenv = require('dotenv');

dotenv.config();

const beetleServerAddr = process.env['BEETLE_SRV_ADDR'] || 'http://0.0.0.0:8337';
const parsedServerAddr = url.parse(beetleServerAddr);

const port = process.env['BEETLE_UI_PROXY_PORT'] || 8338;

const proxy = httpProxy.createProxyServer({});
const buildTargetName = process.argv.includes('--release') ? 'release' : 'debug';
const serveUnder = process.argv.includes('--serve-under');
console.log(`serveUnder? ${serveUnder}`);

const server = http.createServer(function(request, response) {
  if (request.url.startsWith('/api')) {
    console.info(`proxying request '${request.url}' to '${beetleServerAddr}'`);
    const target = {
      host: parsedServerAddr.hostname,
      port: parsedServerAddr.port,
      path: request.url.slice('/api'.length),
    };
    proxy.web(request, response, { target, ignorePath: true }, function (error) {
      console.error(`non-terminal proxy error - ${error.message}`);
    });
    return;
  }

  const resource = serveUnder ? request.url.replace('/beetle', '') : request.url;
  const staticPath = path.join(__dirname, 'target', buildTargetName, resource);

  fs.stat(staticPath, function (error, stats) {
    const resolvedPath = !error && stats.isFile()
      ? staticPath
      : path.join(__dirname, 'target', buildTargetName, 'index.html');

    console.info(`attempting to serve static file from '${resolvedPath}' (from '${staticPath}')`);

    fs.readFile(resolvedPath, function (error, data) {
      if (error) {
        console.warn(`unable to read ${resolvedPath} - ${error}`);
        response.writeHead(500);
        response.end();

        return;
      }

      response.writeHead(200);
      response.end(data);
    });
  });
});

console.info(`development server listening on port '${port}'`)
server.listen(port);

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

const server = http.createServer(function(request, response) {
  if (request.url.startsWith('/api')) {
    console.info(`proxying request '${request.url}' to '${beetleServerAddr}'`);
    const target = {
      host: parsedServerAddr.hostname,
      port: parsedServerAddr.port,
      path: request.url.slice('/api'.length),
    };
    proxy.web(request, response, { target, ignorePath: true });
    return;
  }

  const staticPath = path.join(__dirname, 'target/debug', request.url);

  fs.stat(staticPath, function (error, stats) {
    const resolvedPath = !error && stats.isFile()
      ? staticPath
      : path.join(__dirname, 'target/debug/index.html');

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

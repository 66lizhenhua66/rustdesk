// Standalone, loopback-only UI prototype. No product services or dependencies.
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const port = Number(process.argv[2] || 4318);
const files = new Set(['index.html', 'styles.css', 'host.css', 'app.js', 'host.js', 'session.js', 'session.css', 'keyboard.js', 'files.js', 'files.css', 'DESIGN.md', 'README.md']);
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.md': 'text/plain' };
http.createServer((req, res) => {
  const pathname = new URL(req.url, 'http://localhost').pathname;
  const name = pathname === '/' ? 'index.html' : pathname.slice(1);
  if (!files.has(name)) { res.writeHead(404); res.end('Not found'); return; }
  fs.readFile(path.join(__dirname, name), (error, content) => {
    if (error) { res.writeHead(404); res.end('Not found'); return; }
    res.writeHead(200, { 'Content-Type': `${types[path.extname(name)]}; charset=utf-8`, 'Cache-Control': 'no-store' });
    res.end(content);
  });
}).listen(port, '127.0.0.1', () => console.log(`Remote Desk prototype: http://127.0.0.1:${port}`));

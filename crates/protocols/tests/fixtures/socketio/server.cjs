const http = require('node:http');
const { Server } = require('socket.io');
const fs = require('node:fs');
const server = process.env.TLS_CERT ? require('node:https').createServer({ cert: fs.readFileSync(process.env.TLS_CERT), key: fs.readFileSync(process.env.TLS_KEY), ca: process.env.TLS_CA ? fs.readFileSync(process.env.TLS_CA) : undefined, requestCert: process.env.TLS_REQUIRE_CLIENT === "1", rejectUnauthorized: process.env.TLS_REQUIRE_CLIENT === "1" }) : http.createServer();
const io = new Server(server, { path: '/custom/socket.io/', transports: ['websocket'], pingInterval: 200, pingTimeout: 1000, maxHttpBufferSize: 1024 * 1024 });
io.of('/fixture').use((socket, next) => {
  if (process.env.TLS_HOST && socket.request.socket.servername !== process.env.TLS_HOST) return next(new Error('Original TLS hostname required'));
  if (socket.handshake.auth.token !== 'fixture-token') return next(new Error('Bad auth fixture-token'));
  if (socket.handshake.query.required !== 'yes' || socket.handshake.headers['x-fixture'] !== 'yes') return next(new Error('Missing query/header'));
  next();
}).on('connection', socket => {
  // error/open/close are ordinary Socket.IO application events, not reserved names.
  for (const event of ['echo', 'error', 'open', 'close']) {
    socket.on(event, (...args) => {
      const ack = typeof args.at(-1) === 'function' ? args.pop() : null;
      socket.emit(event, ...args);
      if (ack) ack(...args);
    });
  }
  socket.on('network-info', () => socket.emit('network-info', {host: socket.handshake.headers.host, proxyAuthorization: socket.handshake.headers['proxy-authorization'] ?? null, tlsAuthorized: socket.request.socket.authorized ?? null}));
  socket.on('scope-check', value => socket.emit('echo', { scope_correct: value === 'script-selected', auth_correct: socket.handshake.auth.token === 'fixture-token' }));
  socket.on('ack-error', (_value, ack) => ack({ error: 'application-error', code: 42 }));
  socket.on('ack-timeout', () => {});
  socket.on('server-ack', () => socket.timeout(2500).emit('question', 'answer me', Buffer.from([0, 255, 7]), (err, ...args) => {
    socket.emit('server-ack-result', { error: err?.message || null }, ...args);
  }));
  socket.on('private', value => socket.emit('private', { echo: value, auth: socket.handshake.auth.token }));
  socket.on('close-now', () => socket.disconnect(true));
  socket.on('burst', () => { for (let i = 0; i < 300; i++) socket.emit('echo', 'x'.repeat(4096), i); });
});
server.listen(Number(process.env.PORT || 18886), '127.0.0.1', () => console.log(`ready:${server.address().port}`));
process.on('SIGTERM', () => { io.close(); server.close(); });

#!/usr/bin/env node
/**
 * serve.mjs — zero-dependency static server for this prototype.
 *
 * Why not `python3 -m http.server`: Babel fetches every `text/babel` script with
 * an XMLHttpRequest, so one page load opens a burst of parallel requests. The
 * stdlib server speaks HTTP/1.0 and closes each connection after one response,
 * which under that burst intermittently resets a request — and a dropped
 * `app.jsx` or `direction-*.jsx` leaves the page blank with no visible error.
 * This server speaks HTTP/1.1, keeps connections alive, and sets Content-Length,
 * which is all it takes to make the reloads reliable.
 *
 * Usage: node serve.mjs [port] [root]
 *   node serve.mjs                      # port 4319, serves designs/ (the parent)
 *   node serve.mjs 4319 ..              # explicit
 */

import fs from 'node:fs'
import http from 'node:http'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const port = Number(process.argv[2] || 4319)
// Default to the directory that holds the projects (…/designs), so both this
// prototype and its neighbours are reachable from one origin.
const root = path.resolve(process.argv[3] || path.join(here, '..'))

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.jsx': 'text/babel; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.txt': 'text/plain; charset=utf-8',
}

http
  .createServer((req, res) => {
    let pathname
    try {
      pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname)
    } catch {
      res.writeHead(400).end('bad request')
      return
    }
    let file = path.join(root, pathname)
    // Never serve outside the root.
    if (!file.startsWith(root)) {
      res.writeHead(403).end('forbidden')
      return
    }
    try {
      if (fs.statSync(file).isDirectory()) file = path.join(file, 'index.html')
    } catch {
      res.writeHead(404).end('not found')
      return
    }
    fs.readFile(file, (err, buf) => {
      if (err) {
        res.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' }).end('not found')
        return
      }
      res.writeHead(200, {
        'content-type': TYPES[path.extname(file).toLowerCase()] || 'application/octet-stream',
        'content-length': buf.length,
        'cache-control': 'no-store',
      })
      res.end(buf)
    })
  })
  .listen(port, () => {
    console.log(`serving ${root}`)
    console.log(`→ http://localhost:${port}/listenbli-aggregator/ListenBli%20Aggregator.html`)
  })

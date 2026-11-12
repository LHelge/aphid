---
title: Serving with aphid-server
category: Getting Started
tags:
  - guide
  - deployment
---

`aphid-server` is a small production web server for a site built with `aphid build`. It ships as a multi-arch container image, `ghcr.io/lhelge/aphid-server`, that already holds the server, so a site's image is two lines. It needs no configuration file, speaks plain HTTP/1.1, and is meant to run behind a reverse proxy that terminates TLS.

It is not the dev server: there is no file watching and no live reload. For writing, use `aphid serve` (see [[dev-server]]).

# The Dockerfile

Build the site into `dist/` with `aphid build --precompress` (or the GitHub Action from [[deployment]] with `precompress: true`), then:

```dockerfile
FROM ghcr.io/lhelge/aphid-server:0.5.0
COPY dist /site
```

The image is `scratch` plus the binary (no shell, no CA bundle). It runs as the non-root user `65532`, listens on port 8080 and serves `/site`. Image tags follow aphid's releases (`0.5.0`, `0.5`, `latest`); pin an exact version.

# What it serves

- Files from the root, with `index.html` for directories, content types by extension, `ETag`/`Last-Modified` revalidation (`304 Not Modified`) and byte-range requests.
- Text responses (HTML, CSS, JavaScript, SVG, feeds, …) compressed with brotli or gzip when the request's `Accept-Encoding` allows it, from precompressed files where the build wrote them. See [Compression](#compression).
- A directory requested without a trailing slash redirects to the slashed path with a *relative* `Location` (`/posts` → `/posts/`), so the redirect stays on the public host and scheme behind a proxy.
- Missing paths return status 404 with the site's `404.html` if there is one, a plain-text 404 otherwise.
- `GET /healthz` returns 200 without touching the file system. It is never logged, and it takes precedence over a file with that name.
- `..` segments, encoded or not, cannot escape the root.

On `SIGTERM` or `SIGINT` it stops accepting connections, finishes in-flight requests and exits 0.

# Compression

Text responses are sent compressed, with brotli (`br`) or gzip, whichever the client's `Accept-Encoding` prefers. There is nothing to configure on the server.

- **Precompressed files:** if a file has a `.br` or `.gz` sibling (`index.html.br`, `index.html.gz`), the server sends that file as it is, with its `Content-Length`. `aphid build --precompress` writes these siblings at the highest compression levels; see [[cli]].
- **On the fly:** anything without a matching sibling is compressed per request and sent chunked, without `Content-Length`. Nothing is cached, so this costs CPU on every request. Images other than SVG and bodies under 32 bytes are sent as they are.
- **Uncompressed:** a client that sends no `Accept-Encoding` gets the original file, so the originals must stay in the image.

A compressed response has `Content-Encoding` and `Vary: Accept-Encoding`. A byte-range request applies to whichever file is selected: with an `Accept-Encoding` that matches a precompressed file, the range is taken from the compressed bytes, and each variant has its own `ETag`. If the reverse proxy or CDN in front already compresses, nothing is compressed twice: whichever layer encodes first wins.

A site built without `--precompress` works the same, with everything compressed on the fly. The `.br` and `.gz` files are also reachable under their own URLs (`/index.html.br`), like any other file in the root.

# Options

Every option is a flag or an environment variable; a flag wins over the environment.

| Flag | Environment | Default | Description |
|------|-------------|---------|-------------|
| `--root` | `APHID_ROOT` | `/site` | Directory to serve |
| `--port` | `PORT` | `8080` | Listen port, on all interfaces |
| `--log-format` | `APHID_LOG_FORMAT` | `text` | `text` for humans, `json` for one JSON object per line |
| `--log-level` | `APHID_LOG_LEVEL` | `info` | Log filter, e.g. `warn` or `info,aphid_server=debug` |
| `--otlp-endpoint` | `OTEL_EXPORTER_OTLP_ENDPOINT` | unset | OTLP/HTTP base URL; enables OpenTelemetry export |

# Logs

Logs go to stdout. Every request except `/healthz` produces one `info` event with target `aphid_server::access` and the fields `method`, `path`, `status`, `duration_ms` (time to response headers), `bytes` (the size of the file sent: the compressed size for a precompressed file, the size before compression otherwise), `referrer`, `user_agent` and `client` (the left-most `X-Forwarded-For` entry, else the peer address). With `--log-format json`:

```json
{"timestamp":"2026-11-12T10:24:14.440176Z","level":"INFO","message":"request","method":"GET","path":"/broken-link/","status":404,"duration_ms":0.44,"bytes":14,"referrer":"https://example.com/posts/","user_agent":"Mozilla/5.0 …","client":"203.0.113.7","target":"aphid_server::access"}
```

To find broken links, filter on the status and look at the referrer:

```sh
jq -c 'select(.status == 404) | {path, referrer}' server.log
```

# OpenTelemetry

Setting `--otlp-endpoint` (or `OTEL_EXPORTER_OTLP_ENDPOINT`) exports traces and logs over OTLP/HTTP with protobuf. Without it, no exporter is started. Stdout logging is unchanged either way.

- **Traces:** one server span per request (none for `/healthz`), named after the method and carrying the HTTP semantic-convention attributes (`http.request.method`, `url.path`, `http.response.status_code`, `client.address`, `user_agent.original`, …). `http.response.body.size` is the same number as `bytes` in the access log. An incoming W3C `traceparent` header is continued. Spans are exported whatever `--log-level` is.
- **Logs:** log events that pass `--log-level` are exported as OTel log records, linked to the request's span.
- **Configuration:** the endpoint is a base URL (`/v1/traces` and `/v1/logs` are appended). The standard variables `OTEL_SERVICE_NAME` (default `aphid-server`), `OTEL_RESOURCE_ATTRIBUTES`, `OTEL_EXPORTER_OTLP_HEADERS`, `OTEL_EXPORTER_OTLP_TIMEOUT` and the per-signal `OTEL_EXPORTER_OTLP_{TRACES,LOGS}_ENDPOINT` are honoured.
- **HTTPS endpoints:** certificates are checked against Mozilla's root store, which is compiled into the binary, so the image needs no CA bundle. A collector with a private or self-signed CA is not supported. The server itself only speaks plain HTTP; inbound TLS is the reverse proxy's job.
- **Failure handling:** export runs in the background and drops data when the collector can't keep up. It never delays or fails a request. When export starts failing, one warning is logged, and one more line when it recovers. On shutdown, buffered telemetry is flushed for at most 3 seconds.

# hoster

With [hoster](https://github.com/LHelge/hoster), the site is an ordinary service. hoster injects `PORT=8080` and probes `GET /healthz` by default, and both work with no further setup:

```toml
version = 1
name = "blog"

[services.web]
image = "ghcr.io/acme/blog"        # FROM ghcr.io/lhelge/aphid-server + COPY dist /site

[services.web.env]
APHID_LOG_FORMAT = "json"
# Optional: export traces and logs to a collector on the stack network.
# OTEL_EXPORTER_OTLP_ENDPOINT = "http://collector:4318"

[[services.web.routes]]
host = "blog.example.com"
port = 8080
```

Put auth headers for a hosted collector in `[services.web.secrets]` as `OTEL_EXPORTER_OTLP_HEADERS`.

This documentation site is deployed exactly this way. For a complete working setup, see [`docs/Dockerfile`](https://github.com/LHelge/aphid/blob/main/docs/Dockerfile), the stack file [`docs/hoster/aphid-docs.stack.toml`](https://github.com/LHelge/aphid/blob/main/docs/hoster/aphid-docs.stack.toml) and the workflow [`.github/workflows/docs.yml`](https://github.com/LHelge/aphid/blob/main/.github/workflows/docs.yml), which builds the image and deploys its digest through hoster's deploy webhook.

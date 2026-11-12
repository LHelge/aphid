# A site built with `aphid build --precompress` (output in dist/), served by aphid-server.
# Listens on 8080 (or $PORT), answers /healthz, serves 404.html for missing pages.
FROM ghcr.io/lhelge/aphid-server:0.5.0
COPY dist /site

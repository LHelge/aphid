# ghcr.io/lhelge/aphid-server: the aphid-server binary on scratch.
#
# Built by .github/workflows/release.yml with buildx for linux/amd64 and
# linux/arm64. The static musl binaries are compiled beforehand and placed at
# dist/<TARGETARCH>/aphid-server in this directory; nothing is compiled here.
# No CA bundle is needed: the binary carries its own root store for HTTPS
# OTLP endpoints, and inbound TLS is the reverse proxy's job.
#
# A site's own image is then just:
#   FROM ghcr.io/lhelge/aphid-server:<version>
#   COPY dist /site
FROM scratch
ARG TARGETARCH
COPY --chmod=0755 dist/${TARGETARCH}/aphid-server /aphid-server
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/aphid-server"]

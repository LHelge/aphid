# ghcr.io/lhelge/aphid-server: the aphid-server binary on scratch.
#
# Built by .github/workflows/release.yml with buildx for linux/amd64 and
# linux/arm64. The static musl binaries are compiled beforehand and placed at
# dist/<TARGETARCH>/aphid-server in this directory; nothing is compiled here.
#
# A site's own image is then just:
#   FROM ghcr.io/lhelge/aphid-server:<version>
#   COPY dist /site

# CA certificates for HTTPS OTLP endpoints. Architecture-independent, so this
# stage always runs on the build platform (no emulation).
FROM --platform=$BUILDPLATFORM docker.io/library/alpine:3.22 AS certs
RUN apk add --no-cache ca-certificates

FROM scratch
ARG TARGETARCH
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --chmod=0755 dist/${TARGETARCH}/aphid-server /aphid-server
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
USER 65532:65532
EXPOSE 8080
ENTRYPOINT ["/aphid-server"]

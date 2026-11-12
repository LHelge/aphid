# The aphid documentation site (https://aphid.lhelge.se), served by aphid-server.
#
# Built and deployed to hoster by .github/workflows/docs.yml after aphid has
# rendered the site into docs/dist/. APHID_SERVER_VERSION is the release being
# documented, so the docs always run on the server version they describe.
ARG APHID_SERVER_VERSION
FROM ghcr.io/lhelge/aphid-server:${APHID_SERVER_VERSION}
COPY dist /site

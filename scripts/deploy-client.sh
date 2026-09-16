#!/usr/bin/env bash
# 1. Build the tengri-client image (React SPA behind nginx) on this mashine
# 2. Push it to the prod server's loopback registry over the SSH tunnel
# 3. Restart the service, verify it, and drop the API's cached HTML shell.
#
# The server never builds images. Mirrors deploy-server.sh for the API.
#
# Run from the repo root. Deploy target comes from server/.env
# (PROD_SERVER_SSH_HOST, PROD_SERVER_ORIGIN); see deploy-server.sh for details.
set -euo pipefail

ENV_FILE="server/.env"
read_env() { grep -E "^$1=" "$ENV_FILE" 2>/dev/null | tail -n1 | cut -d= -f2-; }

# Service name
NAME="tengri-web"
SERVER="$(read_env PROD_SERVER_SSH_HOST)"
ORIGIN="$(read_env PROD_SERVER_ORIGIN)"
: "${SERVER:?set PROD_SERVER_SSH_HOST in server/.env}"
: "${ORIGIN:?set PROD_SERVER_ORIGIN in server/.env}"
PLATFORM="linux/arm64"
DOCKERFILE="client/Dockerfile"
# The directory for DOCKERFILE
CONTEXT="client"
# The step in the Dockerfile to build
TARGET="runtime"
# Must be a path nginx serves. Extensionless routes reach the API, which answers
# them from its cached shell — a 200 there would say nothing about this service.
HEALTH_URL="${ORIGIN%/}/index.html"
HEALTH_TIMEOUT=30
# The API caches nginx's index.html, so a new build stays invisible until dropped.
RELOAD_URL="${ORIGIN%/}/api/html/reload"

IMAGE="localhost:5000/${NAME}:latest"

echo "==> build"
docker build --platform "${PLATFORM}" -f "${DOCKERFILE}" --target "${TARGET}" -t "${IMAGE}" "${CONTEXT}"

echo "==> ensure tunnel"
ssh -O check "${SERVER}" 2>/dev/null || ssh -fN "${SERVER}"

echo "==> push"
docker push "${IMAGE}"

echo "==> restart"
ssh -t "${SERVER}" sudo systemctl restart "${NAME}"

echo "==> verify ${HEALTH_URL}"
for ((i = 1; i <= HEALTH_TIMEOUT; i++)); do
  code="$(curl -fsS -o /dev/null -w '%{http_code}' "${HEALTH_URL}" || echo 000)"
  if [[ "${code}" == "200" ]]; then
    echo "==> reload ${RELOAD_URL}"
    curl -fsS -o /dev/null -X POST "${RELOAD_URL}" || {
      echo "ERROR: reload failed; the API keeps serving the previous index.html" >&2
      exit 1
    }
    echo "==> done (healthy after ${i}s)"
    exit 0
  fi
  printf "    waiting... (HTTP %s, %d/%d)\n" "${code}" "${i}" "${HEALTH_TIMEOUT}"
  sleep 1
done

echo "ERROR: ${HEALTH_URL} did not return 200 within ${HEALTH_TIMEOUT}s" >&2
exit 1

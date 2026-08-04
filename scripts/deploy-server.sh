#!/usr/bin/env bash
# 1. Build the tengri-server image on this mashine
# 2. Push it to the prod server's loopback registry over the SSH tunnel
# 3. Restart the "tengri" service, and verify the public health endpoint.
# 
# The server never builds images.
#
# Run from the repo root.
set -euo pipefail

# Deploy target is read from server/.env (gitignored):
#   PROD_SERVER_SSH_HOST — ssh(1) host for the box; its ~/.ssh/config entry must
#     carry `LocalForward 5000 127.0.0.1:5000` so `docker push` reaches the
#     server's loopback registry over the tunnel.
#   PROD_SERVER_ORIGIN   — public https base, used only to poll the health check.
ENV_FILE="server/.env"
read_env() { grep -E "^$1=" "$ENV_FILE" 2>/dev/null | tail -n1 | cut -d= -f2-; }

# Service name
NAME="tengri"
SERVER="$(read_env PROD_SERVER_SSH_HOST)"
ORIGIN="$(read_env PROD_SERVER_ORIGIN)"
: "${SERVER:?set PROD_SERVER_SSH_HOST in server/.env}"
: "${ORIGIN:?set PROD_SERVER_ORIGIN in server/.env}"
PLATFORM="linux/arm64"
DOCKERFILE="server/Dockerfile"
# The directory for DOCKERFILE
CONTEXT="server"
# The step in the Dockerfile to build
TARGET="runtime"
HEALTH_URL="${ORIGIN%/}/api/health"
HEALTH_TIMEOUT=30

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
    echo "==> done (healthy after ${i}s)"
    exit 0
  fi
  printf "    waiting... (HTTP %s, %d/%d)\n" "${code}" "${i}" "${HEALTH_TIMEOUT}"
  sleep 1
done

echo "ERROR: ${HEALTH_URL} did not return 200 within ${HEALTH_TIMEOUT}s" >&2
exit 1

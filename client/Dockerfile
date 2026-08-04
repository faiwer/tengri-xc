# Multi-stage build for the tengri-xc React client. Build context is `client/`.
# Target linux/arm64 (Apple Silicon builder -> Asahi arm64 host).
FROM node:22-bookworm-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
COPY patches ./patches
RUN npm ci
# Bring in the rest: src/, packages/ submodule, public/, config.
COPY . .
RUN echo 'VITE_SERVER_URL=/api' > .env.production && npx vite build

FROM nginx:alpine AS runtime
# curl backs the container HealthCmd (curl .../healthz) used by the Quadlet.
RUN apk add --no-cache curl
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html
EXPOSE 80

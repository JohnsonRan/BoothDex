# Next.js frontend as a standalone server.
#   podman build -f deploy/web.Dockerfile -t boothdex-web .
FROM docker.io/oven/bun:1 AS build
WORKDIR /web
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile
COPY web ./
# the /api and /img rewrites are baked in at build time; these are the compose service names
ENV API_ORIGIN=http://api:3000 IMG_ORIGIN=http://imgproxy:3001
RUN bun run build \
 && { [ ! -d public ] || cp -r public .next/standalone/; } \
 && cp -r .next/static .next/standalone/.next/

FROM docker.io/library/node:26-trixie-slim
WORKDIR /app
COPY --from=build /web/.next/standalone ./
# the home page also reads the API on the server at request time
ENV NODE_ENV=production PORT=3100 HOSTNAME=0.0.0.0 API_ORIGIN=http://api:3000
USER node
EXPOSE 3100
CMD ["node", "server.js"]

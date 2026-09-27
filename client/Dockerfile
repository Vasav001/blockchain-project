# syntax=docker/dockerfile:1

# ---- Builder ----
FROM node:22-slim AS builder
WORKDIR /app

# Cache npm install in its own layer, separate from application source.
COPY package.json package-lock.json ./
RUN npm ci

COPY . .
RUN npm run build

# ---- Runtime ----
# Static files only - no Node.js at runtime. nginx also reverse-proxies
# /api/* to the backend service (see nginx.conf), so the browser only
# ever talks to this one origin.
FROM nginx:1-alpine AS runtime

COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

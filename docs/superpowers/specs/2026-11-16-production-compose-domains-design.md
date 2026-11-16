# Production Compose and domain deployment

## Owner-approved intent

Run the existing frontend, Rust backend and PostgreSQL with Docker Compose on this VPS. Serve the frontend at https://imtsmartteens.my.id and bind https://api.imtsmartteens.my.id. The owner wants frontend-only API access, with exact CORS origin https://imtsmartteens.my.id, and approved the following interpretation: the API hostname has valid TLS but denies direct public access; browsers use the frontend's same-origin /api/v1 proxy instead. CORS is not a firewall and this is not a promise that an HTTP client cannot call public frontend proxy endpoints. Authentication, CSRF and authorization remain mandatory.

The owner approved the architecture in chat on 2026-11-16. This document records that approval; implementation and integration follow the plan.

## Architecture and builder decisions

- Three long-lived services: frontend, backend, postgres. A separate explicit operations profile handles schema migration and initialization; no migration or teaching-data seeding on normal server startup.
- Host Nginx owns ports80/443 and certificate renewal. Preserve the existing rizkyfs.my.id site and its certificates. Add only the two IMT virtual hosts and their ACME challenge directory. Reuse the existing Certbot account; stop and ask if a new account/terms acceptance is required.
- Frontend production image builds with Vite and serves static files plus /api/v1 reverse proxy through non-root Nginx. Vite config retains the development proxy, with Origin preserved and no public Vite preview server.
- Only frontend publishes a host port, bound to 127.0.0.1:5180. Backend4180 and PostgreSQL5432 have no published ports. Internal Docker networks separate frontend/backend traffic from backend/database traffic.
- Public API hostname returns403 for application paths; only ACME validation is exempt. HTTP redirects to HTTPS outside the challenge path. No public backend health/admin route bypass.
- Backend APP_ENV=production, SESSION_COOKIE_SECURE=true and CORS_ALLOWED_ORIGINS=https://imtsmartteens.my.id. Preserve HttpOnly, SameSite, CSRF, school grants, role separation and all current retry/versioning behavior. No cookie-domain broadening, Origin rewriting or wildcard CORS.
- Fresh named production PostgreSQL volume/database; preserve all existing foundation/application/E2E databases and volumes. No fixtures, demo accounts or teaching content are copied into production. Initial state contains reviewed schema and explicitly initialized empty bank/path roots only.
- Separate limited migrator and DML-only runtime roles. A private explicit SQL operations step initializes empty roots using the existing schema/invariants, rather than bypassing the development-only CLI guard or adding a public bootstrap endpoint. Admin provisioning/content/report grants remain separate owner operations.
- Secret files stay outside Git and Docker build contexts, under /home/ubuntu/.config/imt-smart-teens/production. Root/operator-only setup creates service-readable400 files with appropriate numeric ownership; directory700. Runtime receives only its own database URL/CSRF key, never a superuser or migrator secret. PostgreSQL bootstrap and migrator secrets mount only into their intended services. No credential values, cookies or authentication artifacts in logs/artifacts/chat.
- The existing backend limiter currently uses socket-peer IP; proxies would otherwise combine every visitor. Add an optional exact TRUSTED_PROXY_IPS allowlist (default empty). Only a listed socket peer may supply one valid X-Real-IP; malformed/duplicate headers fall back to peer IP. Never consume arbitrary X-Forwarded-For/Forwarded. Host Nginx overwrites client-supplied IP headers; frontend forwards that sanitized value.
- Reserve Docker app subnet172.30.251.0/28 after rechecking conflicts; frontend172.30.251.2 is the sole trusted backend proxy, backend172.30.251.3. Builder must select another explicit non-overlapping subnet and document it if a conflict appears. Current host routes/networks do not overlap.
- Resolve and pin official base-image digests during implementation. Keep the reviewed Rust1.100.0 toolchain; lock frontend dependencies. Non-root frontend/backend, read-only application filesystems with required tmpfs, dropped capabilities, healthchecks and restart policy. Build serially on this approximately2GiB host.

## Acceptance and operational boundaries

- Build actual images, run an isolated throwaway smoke stack first, prove role restrictions/migration idempotence/private networking/cookie+Origin behavior, then start the production stack with an empty production database.
- Preserve secrets/data across container recreation; never run down -v against production or existing projects. Public cutover occurs only after health and local Host-header smoke pass.
- Validate real DNS, HTTPS certificate SANs, HTTP redirects, frontend assets/deep links, proxied API, denied foreign Origin, blocked direct API hostname and unchanged portfolio. No claim of production enrollment/content readiness without privately provisioned real admin/school/content.
- DNS currently resolves both new names and the portfolio to43.133.132.59. Docker Compose and Certbot are installed. These are observations, not substitutes for deployment-time checks.
- No automatic push/merge for this new milestone. Commit reviewed deployment work on isolated branches; owner chooses integration. Live deployment/configuration changes are authorized by this request. Do not stop the existing local demo or database just to free convenient ports.

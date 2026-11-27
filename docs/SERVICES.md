# Service behaviour

Reference for how `imgproxy`, `api` and `web` behave at their edges. For the steps that set them up, see the [deployment guide](DEPLOY.md). Numbers are in [Measurements](MEASUREMENTS.md).

## Image proxy

- `imgproxy` fetches only from `booth.pximg.net`, `asset.booth.pm` and `factory.pixiv.net`, over https, and follows no redirect off the host.
- It ignores `HTTP_PROXY`, `HTTPS_PROXY` and the system proxy settings.
- It connects only to public addresses. When DNS answers with only private, loopback or other special-purpose addresses, the request fails, the client gets a 502, and the log names the host. NAT64 addresses (`64:ff9b::/96`) count as special-purpose, so the container needs IPv4 or native IPv6 egress.
- It holds at most 200 MiB of uncached upstream images in memory at once, counting each by its declared size (25 MiB when it declares none). A request that finds no room within 10 s gets a 503 with `Retry-After: 10`.
- Each client (an IPv4 address or an IPv6 /64, found as `api` finds it; see "Client address" below) may cause 100 upstream requests a second, in bursts of 2,000. That is enough for fast scrolling through results where no image is cached yet. A cache miss past that gets a 429 with `Retry-After` and `Cache-Control: no-store`, and fetches nothing. Cache hits, in `imgproxy` or the CDN, are never limited.
- The number of upstream requests in flight from all clients together is not bounded.

## One `api` container

Run exactly one `api` container. Each `api` process keeps its own key for signing the stricter limits' `pass` cookie, its own copy of the AI and Turnstile settings and its own rate-limit buckets. With two of them, a pass issued by one is refused by the other, settings saved through one do not reach the other until it restarts, and every per-address limit doubles.

## Client address

`api`'s public read endpoints and account changes, and `imgproxy`'s cache misses, key their per-IP rate limits on `X-Forwarded-For` or `CF-Connecting-IP` from the front proxy, which `web` passes on unchanged.

- The front proxy must set or append `X-Forwarded-For` to the verified client IP. Caddy's `reverse_proxy` does this; nginx needs `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;`. Setting only `X-Real-IP` without sanitizing an incoming `X-Forwarded-For` lets visitors forge their rate-limit identity.
- `api` never rate-limits a request whose client IP is internal (loopback, private or link-local): that is `web`'s own reads, a LAN visitor or local development. A proxy that sets none of these headers therefore makes every visitor count as `web`'s own address, so no visitor is rate-limited. `api` then logs `warn: a browser's request reached api with no visitor address` at most once a minute.
- A forwarding header holding an entry that is not an address, with no usable visitor address in any of them, puts the request in one bucket shared by all such requests, shown as `100::` where the client IP is printed or stored.
- The address the rate limits key on is printed in the sign-in log. It is also stored with each new session and shown in Settings, under devices, and in the admin user view. When that address is internal, the session stores no address. A proxy that does not set `X-Forwarded-For` therefore also lets a visitor falsify the sign-in log and the stored session address.

## Content Security Policy

`web` sends `Content-Security-Policy: img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'` on every response.

- The browser refuses an image from any other host, including one a proxy injects.
- It loads no plugin content, accepts no `<base>` element, and shows no page of the site inside a frame on any site, this one included.
- Scripts and styles are not restricted.
- Every response also sends `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy: camera=(), geolocation=(), microphone=()`, and `Cross-Origin-Opener-Policy: same-origin-allow-popups`.
- A policy your proxy adds does not replace this one: the browser enforces both.

## Page titles

Catalogue view titles use the same filters and language as the visible heading. The unfiltered catalogue keeps the
site's default metadata title. Admin titles use the console's translations, with English as the fallback for Korean.

## Cloudflare Turnstile

The keys are those of a Cloudflare Turnstile widget in managed mode. They come from `TURNSTILE_SITE_KEY` and `TURNSTILE_SECRET_KEY` or from `/admin`; the environment values override and lock the `/admin` settings.

- **Guestbook:** with either key unset in both places, the guestbook shows that it is not open and refuses posts.
- **Sign-in and registration:** a switch in the same settings makes both carry the widget. It does nothing while a key is unset. A check Cloudflare cannot make (unreachable, or a wrong secret key) lets the request through with an error in the `api` log. A wrong site key, or a site key and secret key of two different widgets, is different: the widget gives no token or Cloudflare refuses every token, and nobody signs in. So turning the switch on, or saving a key while it is on, first asks the admin to pass the widget in the console with the keys being saved. That check proves the keys only for the hostname the admin's browser is on, since a widget works only on the hostnames listed for it at Cloudflare and their subdomains; keys from `TURNSTILE_*` change at a restart, outside it. The [deployment guide](DEPLOY.md#6-operations) gives the command that turns this switch off from the host.
- **Stricter limits:** a second switch, "Stricter limits for visitors who haven't verified", gives `/api/search`, `/api/palette`, `/api/items`, the gallery and the lookups (the guestbook list included) a stricter per-address bucket for visitors without a pass. `crates/api/src/limit.rs` holds both tiers' numbers.
  - Without a pass, `/api/search` and `/api/items` with several ids also pay per record from one bucket per address, 15 records a second with a burst of 1,200 (a search its `pageSize`, a list its ids), so a long scroll on a 3840x2160 window is offered the widget after about a minute.
  - A refusal there offers the widget. Passing it sets a `pass` cookie, valid for 10 minutes from the visitor's IPv4 /24 or IPv6 /64, that restores the usual limits.
  - Without a pass, `/api/rss` pays its 50 items from the same record bucket and is refused with a plain 429 when that bucket is empty, since a feed reader cannot pass the widget, and no session is read for it. A feed reader behind the same address as a browsing visitor shares that visitor's budget.
  - While the sign-in switch is on as well, a session also gets the usual limits if the sign-in or registration that made it passed the widget (`sessions.turnstile_passed`). A session let through unchecked because Cloudflare could not judge the token, or made while the sign-in switch was off, counts as no pass. With the sign-in switch off, every session counts as no pass.
  - A restart of `api` ends every pass.
- **When changes apply:** for the stricter-limits switch and the guestbook, `api` reads the row at start and when the console saves it, so restart `api` after changing the row by hand. The sign-in switch is read from the row on every request and needs no restart.

## Caching

- `/img/*` responses carry `Cache-Control: public, max-age=31536000, immutable`.
- `/api/items` (item records, the item card's first request) carries `Cache-Control: public, max-age=43200` (12 hours).
- The catalogue lookups (`/api/avatars`, `/api/authors`, `/api/tags`, `/api/booth-tags`, `/api/categories`) carry `Cache-Control: public, max-age=300`.
- `/api/items/{id}/gallery` counts a view on every request, and the account endpoints answer per visitor: cache neither.
- SSE responses carry `Cache-Control: no-transform`.

## Cleanup

- The `worker` deletes expired login sessions and job history (including job errors and logs) older than 90 days every hour.
- Container logs are capped by `x-logging` in `compose.yaml`: Docker keeps 3 × 10 MB per service, and Podman truncates each log at 10 MB.
- The `embed` job prunes `query_embeddings` (see [Background jobs](WORKER.md#semantic-search-vectors)).
- Autovacuum handles PostgreSQL; no manual `VACUUM` is needed.
- `imgcache` (re-fetched from BOOTH on demand) and `textindex` (rebuilt by `api` at startup) are safe to lose.

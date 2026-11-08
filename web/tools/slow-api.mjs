// Slow-API mode for the loading checks (lab only, never part of a build): a proxy between a `next start` server
// and the real API. Point the server's run-time API_ORIGIN at it; it forwards every request unchanged (GET only,
// so production is read, never written) and holds `/api/search` back for the current delay. The checks set the
// delay between steps, so one server serves a fast first load and a slow next page:
//   node tools/slow-api.mjs [port=3298] [upstream=https://boothdex.ihtw.moe, or the lab API http://127.0.0.1:3400]
//   curl 127.0.0.1:3298/__lab?delay=2500        search answers 2.5 s late (0: no delay)
//   curl 127.0.0.1:3298/__lab?fail=1            search answers 500 (the API is up, so the page shows "failed")
//   curl 127.0.0.1:3298/__lab?cache=1           a search asked before answers at once from memory (a quick
//                                               refresh: the page renders in well under 150 ms)
import http from "node:http";
import https from "node:https";

const port = Number(process.argv[2] ?? 3298);
const upstream = new URL(process.argv[3] ?? "https://boothdex.ihtw.moe");
// a local lab API (http://127.0.0.1:3400) is plain http
const client = upstream.protocol === "http:" ? http : https;
const agent = new client.Agent({ keepAlive: true, maxSockets: 32 });
const lab = { delay: 0, fail: false, cache: false };
/** search answers by path, kept while `cache` is on */
const answers = new Map();

http
  .createServer((req, res) => {
    const url = new URL(req.url ?? "/", "http://lab");
    if (url.pathname === "/__lab") {
      if (url.searchParams.has("delay"))
        lab.delay = Number(url.searchParams.get("delay")) || 0;
      if (url.searchParams.has("fail"))
        lab.fail = url.searchParams.get("fail") === "1";
      if (url.searchParams.has("cache")) {
        lab.cache = url.searchParams.get("cache") === "1";
        if (!lab.cache) answers.clear();
      }
      res.writeHead(200, { "content-type": "application/json" });
      return res.end(JSON.stringify(lab));
    }
    if (req.method !== "GET") {
      res.writeHead(405);
      return res.end();
    }
    const search = url.pathname === "/api/search";
    const kept = search && lab.cache && answers.get(req.url);
    if (kept && !lab.fail) {
      res.writeHead(200, kept.headers);
      return res.end(kept.body);
    }
    setTimeout(
      () => {
        if (search && lab.fail) {
          res.writeHead(500);
          return res.end();
        }
        const up = client.request(
          {
            hostname: upstream.hostname,
            port: upstream.port || undefined,
            path: req.url,
            method: "GET",
            agent,
            headers: { ...req.headers, host: upstream.host },
          },
          (r) => {
            res.writeHead(r.statusCode ?? 502, r.headers);
            if (search && lab.cache && r.statusCode === 200) {
              const parts = [];
              r.on("data", (c) => parts.push(c));
              r.on("end", () =>
                answers.set(req.url, {
                  headers: r.headers,
                  body: Buffer.concat(parts),
                }),
              );
            }
            r.pipe(res);
          },
        );
        up.on("error", () => {
          res.writeHead(502);
          res.end();
        });
        up.end();
      },
      search ? lab.delay : 0,
    );
  })
  .listen(port, "127.0.0.1");

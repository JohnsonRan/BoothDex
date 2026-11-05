// Slow-API mode for the loading checks (lab only, never part of a build): a proxy between a `next start` server
// and the real API. Point the server's run-time API_ORIGIN at it; it forwards every request unchanged (GET only,
// so production is read, never written) and holds `/api/search` back for the current delay. The checks set the
// delay between steps, so one server serves a fast first load and a slow page change:
//   node tools/slow-api.mjs [port=3298] [upstream=https://boothdex.ihtw.moe]
//   curl 127.0.0.1:3298/__lab?delay=2500        search answers 2.5 s late (0: no delay)
//   curl 127.0.0.1:3298/__lab?fail=1            search answers 500 (the API is up, so the page shows "failed")
import http from "node:http";
import https from "node:https";

const port = Number(process.argv[2] ?? 3298);
const upstream = new URL(process.argv[3] ?? "https://boothdex.ihtw.moe");
const agent = new https.Agent({ keepAlive: true, maxSockets: 32 });
const lab = { delay: 0, fail: false };

http
  .createServer((req, res) => {
    const url = new URL(req.url ?? "/", "http://lab");
    if (url.pathname === "/__lab") {
      if (url.searchParams.has("delay"))
        lab.delay = Number(url.searchParams.get("delay")) || 0;
      if (url.searchParams.has("fail"))
        lab.fail = url.searchParams.get("fail") === "1";
      res.writeHead(200, { "content-type": "application/json" });
      return res.end(JSON.stringify(lab));
    }
    if (req.method !== "GET") {
      res.writeHead(405);
      return res.end();
    }
    const search = url.pathname === "/api/search";
    setTimeout(
      () => {
        if (search && lab.fail) {
          res.writeHead(500);
          return res.end();
        }
        const up = https.request(
          {
            host: upstream.host,
            path: req.url,
            method: "GET",
            agent,
            headers: { ...req.headers, host: upstream.host },
          },
          (r) => {
            res.writeHead(r.statusCode ?? 502, r.headers);
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

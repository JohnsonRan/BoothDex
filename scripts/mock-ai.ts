// Fake OpenAI-compatible provider for testing the AI jobs without a key or cost.
//   bun scripts/mock-ai.ts            -> http://127.0.0.1:3555/v1  (any API key works)
//   MOCK_PARTIAL=1 bun scripts/mock-ai.ts   enrich replies carry titles only (no bio/variations),
//                                            to check that existing translations survive
// /v1/models lists one model; /v1/chat/completions answers the two prompts our worker sends:
//   - localize: echoes every key with a "【中】" prefix, so translated labels are easy to spot
//   - enrich:   a fixed, valid enrichment using the first category slug in the prompt
const port = Number(process.env.PORT ?? 3555);
const partial = process.env.MOCK_PARTIAL === "1";

Bun.serve({
  port,
  hostname: "127.0.0.1",
  async fetch(req) {
    const url = new URL(req.url);
    if (url.pathname === "/v1/models") return Response.json({ data: [{ id: "mock-model" }] });
    if (url.pathname !== "/v1/chat/completions" || req.method !== "POST") return new Response("not found", { status: 404 });
    const body = await req.json();
    const system: string = body.messages[0].content;
    const user: string = body.messages[1].content;
    let reply: unknown;
    if (system.includes("Simplified Chinese")) {
      const input = JSON.parse(user) as Record<string, string>;
      reply = Object.fromEntries(Object.keys(input).map((k) => [k, `【中】${k}`]));
    } else {
      const slug = user.match(/^([a-z0-9-]+--[a-z0-9-]+):/m)?.[1] ?? "none";
      const title = user.match(/^Title: (.*)$/m)?.[1] ?? "";
      const t = (prefix: string) => (partial ? { title: `${prefix}${title}` } : { title: `${prefix}${title}`, bio: "mock bio", variations: {} });
      reply = {
        categories: [slug],
        content_rating: "SFW",
        quest_compatible: "No",
        smart_price: 500,
        smart_price_reason: "mock",
        translations: { en: t("[en] "), ja: t(""), ko: t("[ko] ") },
        ...(partial ? {} : { original_language: "ja" }),
        features: ["liltoon"],
      };
    }
    // wrapped in a code fence on purpose: real models do this and the worker must cope
    return Response.json({ choices: [{ message: { content: "```json\n" + JSON.stringify(reply) + "\n```" } }] });
  },
});
console.log(`mock AI on http://127.0.0.1:${port}/v1${partial ? " (partial replies)" : ""}`);

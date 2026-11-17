import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { format } from "@/lib/messages";
import { GivenUpList } from "./given-up";
import type { AdminDict, AdminT } from "./messages";
import { adminEn } from "./messages-en";
import { adminJa } from "./messages-ja";
import { adminZh } from "./messages-zh";
import type { GivenUpItems } from "./types";

const LOCALES: [string, AdminDict][] = [
  ["zh", adminZh],
  ["en", adminEn],
  ["ja", adminJa],
];
const tOf =
  (dict: AdminDict): AdminT =>
  (key, vars) =>
    format(dict[key], vars);
const render = (dict: AdminDict, page: GivenUpItems) =>
  renderToStaticMarkup(
    <GivenUpList
      page={page}
      onRetry={() => {}}
      onRetryAll={() => {}}
      t={tOf(dict)}
    />,
  );
/** The rendered text, tags dropped and React's escapes undone. */
const text = (html: string) =>
  html
    .replace(/<[^>]+>/g, " ")
    .replaceAll("&#x27;", "'")
    .replaceAll("&quot;", '"')
    .replaceAll("&amp;", "&");

test("an empty list says so in every locale, with no retry", () => {
  for (const [lang, dict] of LOCALES) {
    const html = text(render(dict, { attempts: 3, total: 0, items: [] }));
    expect(html, lang).toContain(dict["givenUp.empty"]);
    expect(html, lang).not.toContain(dict["givenUp.retryAll"]);
  }
});

test("a long title is shown whole, and a capped list says how many it shows", () => {
  const title = "とても長い商品名 ".repeat(30).trim();
  for (const [lang, dict] of LOCALES) {
    const html = render(dict, {
      attempts: 3,
      total: 250,
      items: [{ id: 42, title, author_name: "shop", failures: 3 }],
    });
    expect(text(html), lang).toContain(title);
    expect(html, lang).toContain('href="/?item=42"');
    expect(text(html), lang).toContain(
      format(dict["givenUp.shown"], { shown: 1, n: 250 }),
    );
    expect(text(html), lang).toContain(
      format(dict["givenUp.failures"], { n: 3 }),
    );
  }
});

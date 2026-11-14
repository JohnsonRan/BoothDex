// Where the home page's first-load bytes went between two builds: reads two `next analyze export` snapshots and
// prints, per npm package and per source directory, the compressed size of the modules in the client chunks `/`
// loads before any lazy import (the bootstrap and render-dependent groups), and what changed. Run it when
// `check:size` reads over budget, before a budget raise is proposed.
//   on the base:   bunx next analyze --output --snapshot base && bunx next analyze export --snapshot base > base.jsonl
//   on the change: the same with `head`, then: node tools/size-diff.mjs base.jsonl head.jsonl
// The analyzer makes its own build: its chunk names differ from `next build`'s, and its per-module sizes sum higher
// than the gzip -9 bytes `check:size` reads (554,067 B of JS against 521,999 B on 2026-11-14), so the totals here
// explain a reading of `check:size` and never replace it.
import { readFileSync } from "node:fs";

const [before, after] = process.argv.slice(2);
if (!before || !after) {
  console.error("usage: node tools/size-diff.mjs <base.jsonl> <head.jsonl>");
  process.exit(2);
}

/** node_modules/<package> or the first two directories under src/; anything else as written. */
const owner = (path) => {
  const pkg = path.match(/node_modules\/((?:@[^/]+\/)?[^/]+)/);
  if (pkg) return pkg[1];
  const src = path.match(/^\[project\]\/(src\/(?:[^/]+\/)?[^/]+)/);
  return src ? src[1] : path.replace(/^\[project\]\//, "");
};

/** Compressed bytes per owner and per kind (js, css) in the first-load client chunks of `/`. */
function read(file) {
  const rows = readFileSync(file, "utf8")
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  const first = new Set();
  for (const r of rows)
    if (r.type === "group" && r.route === "/" && r.kind !== "async")
      for (const o of r.outputs)
        if (o.includes("/static/chunks/")) first.add(o);
  if (!first.size) {
    console.error(`${file}: no first-load chunks for /`);
    process.exit(2);
  }
  const by = new Map();
  const total = { js: 0, css: 0 };
  for (const r of rows) {
    if (r.type !== "part" || r.route !== "/" || !first.has(r.filename))
      continue;
    const kind = r.filename.endsWith(".css") ? "css" : "js";
    total[kind] += r.compressed_size;
    const key = `${kind} ${owner(r.source_path)}`;
    by.set(key, (by.get(key) ?? 0) + r.compressed_size);
  }
  return { by, total };
}

const a = read(before);
const b = read(after);
const keys = new Set([...a.by.keys(), ...b.by.keys()]);
const rows = [...keys]
  .map((key) => {
    const was = a.by.get(key) ?? 0;
    const now = b.by.get(key) ?? 0;
    return { key, was, now, change: now - was };
  })
  .filter((r) => r.change !== 0)
  .sort((x, y) => Math.abs(y.change) - Math.abs(x.change));

for (const kind of ["js", "css"])
  console.log(
    `${kind}: ${a.total[kind]} -> ${b.total[kind]} (${b.total[kind] - a.total[kind] >= 0 ? "+" : ""}${b.total[kind] - a.total[kind]})`,
  );
console.table(
  rows.slice(0, 20).map(({ key, was, now, change }) => ({
    owner: key,
    base: was,
    head: now,
    change,
  })),
);

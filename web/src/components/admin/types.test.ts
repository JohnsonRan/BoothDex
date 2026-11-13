import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { KINDS } from "./types";

// crates/core/src/jobs.rs is the one list of job kinds; the console keeps a copy
test("the console lists the job kinds of crates/core/src/jobs.rs", () => {
  const rust = readFileSync(
    join(import.meta.dir, "../../../../crates/core/src/jobs.rs"),
    "utf8",
  );
  const arms = [...rust.matchAll(/Self::\w+ => "(\w+)",/g)].map((m) => m[1]);
  expect(arms.length).toBeGreaterThan(0);
  expect([...KINDS].map(String).sort()).toEqual(arms.sort());
});

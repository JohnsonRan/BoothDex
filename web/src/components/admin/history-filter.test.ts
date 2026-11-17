import { expect, test } from "bun:test";
import { format } from "@/lib/messages";
import {
  ALL,
  countLine,
  jobsPath,
  matches,
  UNRESOLVED,
} from "./history-filter";
import type { AdminT } from "./messages";
import { adminEn } from "./messages-en";

test("status=unresolved asks the API for the counted runs; other filters read the last 100 jobs", () => {
  expect(jobsPath(UNRESOLVED)).toBe("/api/admin/jobs?unresolved=true");
  for (const status of [ALL, "failed", "succeeded"])
    expect(jobsPath(status)).toBe("/api/admin/jobs");
});

test("the browser filters by kind and status, and keeps every row the API chose as unresolved", () => {
  const job = { kind: "refresh", status: "failed" } as const;
  expect(matches(job, ALL, ALL)).toBe(true);
  expect(matches(job, "refresh", "failed")).toBe(true);
  expect(matches(job, "details", ALL)).toBe(false);
  expect(matches(job, ALL, "succeeded")).toBe(false);
  expect(matches(job, ALL, UNRESOLVED)).toBe(true);
  expect(matches(job, "details", UNRESOLVED)).toBe(false);
});

const tEn: AdminT = (key, vars) => format(adminEn[key], vars);

test("the count line tells the true total when the API sent only the newest unresolved runs", () => {
  expect(countLine(tEn, 3, 3, true, 3)).toBe(
    "3 unresolved failures in 30 days",
  );
  expect(countLine(tEn, 100, 100, true, 140)).toBe(
    "Newest 100 of 140 unresolved failures in 30 days",
  );
  expect(countLine(tEn, 2, 100, true, 140)).toBe("2 of 100 jobs");
  expect(countLine(tEn, 100, 100, false)).toBe("Last 100 jobs");
});

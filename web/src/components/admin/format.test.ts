import { describe, expect, test } from "bun:test";
import { format } from "@/lib/messages";
import { formatBytes, formatSeconds, intervalLabel, jobEta } from "./format";
import type { AdminT } from "./messages";
import { adminEn } from "./messages-en";
import { adminZh } from "./messages-zh";
import type { Job } from "./types";

const t: AdminT = (key, vars) => format(adminEn[key], vars);

describe("admin format", () => {
  test("every Chinese key has an English string", () => {
    expect(Object.keys(adminEn).sort()).toEqual(Object.keys(adminZh).sort());
  });

  test("bytes step by 1024 and keep one decimal below 100", () => {
    expect(formatBytes(512, "en-US")).toBe("512 B");
    expect(formatBytes(1536, "en-US")).toBe("1.5 KB");
    expect(formatBytes(200 * 1024 ** 2, "en-US")).toBe("200 MB");
    expect(formatBytes(null, "en-US")).toBe("—");
  });

  test("durations pick the largest unit", () => {
    expect(formatSeconds(42, t)).toBe("42 s");
    expect(formatSeconds(125, t)).toBe("2 min 5 s");
    expect(formatSeconds(3 * 3600 + 120, t)).toBe("3 h 2 min");
  });

  test("the 8 presets have short labels", () => {
    expect([15, 180, 1440, 10080].map((m) => intervalLabel(m, t))).toEqual([
      "Every 15 min",
      "Every 3 h",
      "Daily",
      "Weekly",
    ]);
  });

  test("ETA follows the pace so far and waits for one step", () => {
    const job = {
      started_at: "2026-11-06T00:00:00Z",
      done: 10,
      total: 40,
    } as Job;
    const now = Date.parse("2026-11-06T00:01:00Z");
    expect(jobEta(job, t, now)).toBe("3 min 0 s");
    expect(jobEta({ ...job, done: 0 }, t, now)).toBeNull();
  });
});

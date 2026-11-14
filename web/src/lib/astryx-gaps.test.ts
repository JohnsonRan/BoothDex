import { expect, test } from "bun:test";
import en from "@astryxdesign/core/locales/en.json";
import { ASTRYX_GAPS } from "./astryx-gaps";

const source = en as Record<string, { defaultMessage: string }>;
// the argument names a message takes, plural and number arguments included
const args = (m: string) =>
  [...m.matchAll(/\{\s*(\w+)\s*[,}]/g)].map((x) => x[1]).sort();

test("each filled-in string is a library key and takes the same arguments", () => {
  for (const [ui, messages] of Object.entries(ASTRYX_GAPS)) {
    expect(Object.keys(messages).sort()).toEqual(
      Object.keys(ASTRYX_GAPS.zh).sort(),
    );
    for (const [key, message] of Object.entries(messages)) {
      expect(source[key], key).toBeDefined();
      expect([...new Set(args(message))], `${ui} ${key}`).toEqual([
        ...new Set(args(source[key].defaultMessage)),
      ]);
    }
  }
});

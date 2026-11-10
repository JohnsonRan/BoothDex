import { expect, test } from "bun:test";
import type { Translations } from "@/lib/api-types";
import {
  commonPrice,
  downloadFormats,
  downloadName,
  downloadUrl,
  initialLang,
  itemHref,
  itemParam,
  langTabs,
  shopHost,
  variationName,
  variationState,
} from "./item-data";

const translated = {
  translations: {
    zh: { title: "标题", variations: { 通常版: "普通版" } },
    ko: { title: "제목" },
  } as Translations,
};

test("language tabs: original, then only translated languages in tab order", () => {
  expect(langTabs(translated)).toEqual(["original", "zh", "ko"]);
  expect(langTabs({ translations: {} as Translations })).toEqual(["original"]);
});

test("initial tab follows the item-text setting only when translated", () => {
  expect(initialLang(translated, "zh")).toBe("zh");
  expect(initialLang(translated, "en")).toBe("original");
  expect(initialLang(translated, "original")).toBe("original");
});

test("variation names fall back to the original", () => {
  expect(variationName(translated, "通常版", "zh")).toBe("普通版");
  expect(variationName(translated, "通常版", "ko")).toBe("通常版");
  expect(variationName(translated, "通常版", "original")).toBe("通常版");
});

test("variation states", () => {
  expect(variationState("addable_to_cart")).toBe("available");
  expect(variationState("free_download")).toBe("available");
  expect(variationState("sold_out")).toBe("soldOut");
  expect(variationState("restock_not_requested")).toBe("soldOut");
  expect(variationState("closed")).toBe("unavailable");
});

test("one shared price, or null when variations differ", () => {
  const v = (price_jpy: number | null) => ({
    id: 1,
    name: "",
    price_jpy,
    status: "",
    type: "",
  });
  expect(commonPrice([v(500), v(500)])).toBe(500);
  expect(commonPrice([v(null), v(0)])).toBe(0);
  expect(commonPrice([v(500), v(800)])).toBeNull();
});

test("downloads: archived copies are WebP only, names count from 1", () => {
  expect(downloadFormats("/img/v/abc.webp")).toEqual(["webp"]);
  const orig = "/img/b/orig/x.jpg?format=avif";
  expect(downloadFormats(orig)).toEqual(["avif", "png", "jpeg"]);
  expect(downloadUrl(orig, "png")).toBe("/img/b/orig/x.jpg?format=png");
  expect(downloadUrl(orig, "avif")).toBe(orig);
  expect(downloadUrl("/img/v/abc.webp", "webp")).toBe("/img/v/abc.webp");
  expect(downloadName(42, 0, "jpeg")).toBe("booth-42-1.jpg");
});

test("shop host", () => {
  expect(shopHost("https://shop.booth.pm/")).toBe("shop.booth.pm");
  expect(shopHost("not a url")).toBe("not a url");
});

test("?item= is read as a positive id and written without touching other pairs", () => {
  expect(itemParam("?item=12")).toBe(12);
  expect(itemParam("?item=-3")).toBe(0);
  expect(itemParam("?item=x")).toBe(0);
  const base = "https://b.example/?tags=a+b&q=%E5%A5%B3";
  expect(itemHref(base, 7)).toBe(
    "https://b.example/?tags=a+b&q=%E5%A5%B3&item=7",
  );
  expect(itemHref(`${base}&item=7`, 9)).toBe(
    "https://b.example/?tags=a+b&q=%E5%A5%B3&item=9",
  );
  expect(itemHref("https://b.example/?item=7", 0)).toBe("https://b.example/");
});

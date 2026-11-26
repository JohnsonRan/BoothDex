import { afterEach, expect, mock, test } from "bun:test";
import {
  type LinkComponentType,
  useLinkComponent,
} from "@astryxdesign/core/Link";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { MESSAGES } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS, SETTINGS_COOKIE } from "@/lib/settings-core";

// the server renderer has no transitions; run the refresh at once
mock.module("react", () => ({
  ...React,
  useTransition: () => [false, (run: () => void) => run()],
}));
const { AppLink, Providers, useUi } = await import("./providers");

const realDocument = globalThis.document;
afterEach(() => {
  globalThis.document = realDocument;
});

let update: ReturnType<typeof useUi>["update"] = () => {};
function Grab() {
  update = useUi().update;
  return null;
}

test("a second change made before the page renders again keeps the first", () => {
  renderToStaticMarkup(
    <AppRouterContext value={{ refresh() {} } as AppRouterInstance}>
      <Providers settings={DEFAULT_SETTINGS} messages={MESSAGES.zh}>
        <Grab />
      </Providers>
    </AppRouterContext>,
  );
  let cookie = "";
  globalThis.document = {
    documentElement: {},
    set cookie(v: string) {
      cookie = v;
    },
  } as Document;
  update({ ui: "en" });
  update({ currency: "USD" });
  const saved = JSON.parse(
    decodeURIComponent(
      cookie.slice(`${SETTINGS_COOKIE}=`.length).split(";")[0],
    ),
  );
  expect(saved).toEqual({ ...DEFAULT_SETTINGS, ui: "en", currency: "USD" });
});

let provided: LinkComponentType | null = null;
function GrabLink() {
  provided = useLinkComponent();
  return null;
}

test("Providers supplies AppLink with prefetch disabled by default", () => {
  provided = null;
  renderToStaticMarkup(
    <AppRouterContext value={{ refresh() {} } as AppRouterInstance}>
      <Providers settings={DEFAULT_SETTINGS} messages={MESSAGES.zh}>
        <GrabLink />
      </Providers>
    </AppRouterContext>,
  );
  expect((provided as { displayName?: string } | null)?.displayName).toBe(
    "SafeLink(AppLink)",
  );
  expect(AppLink({ href: "/?v=favourites" }).props.prefetch).toBe(false);
  expect(AppLink({ href: "/", prefetch: true }).props.prefetch).toBe(true);
});

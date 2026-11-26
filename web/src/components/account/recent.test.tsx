import { expect, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import { renderToStaticMarkup } from "react-dom/server";
import { AccountProvider } from "@/components/account/session";
import { item } from "@/components/catalog/results/results-grid.test";
import { Providers } from "@/components/providers";
import type { ItemResponse } from "@/lib/api-types";
import { browserMessages, MESSAGES, translator } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { RecentContent } from "./recent";

const renderRecent = (items: ItemResponse[], signedOut: boolean) =>
  renderToStaticMarkup(
    <AppRouterContext value={{} as AppRouterInstance}>
      <Providers settings={DEFAULT_SETTINGS} messages={browserMessages("en")}>
        <AccountProvider>
          <RecentContent
            items={items}
            signedOut={signedOut}
            onSignIn={() => {}}
            settings={{ ...DEFAULT_SETTINGS, ui: "en" }}
            rates={{}}
            t={translator("en")}
          />
        </AccountProvider>
      </Providers>
    </AppRouterContext>,
  );

test("signed-out recent views show local-storage sync prompt in empty and populated states", () => {
  const empty = renderRecent([], true);
  expect(empty).toContain(MESSAGES.en["recent.empty"]);
  expect(empty).toContain(MESSAGES.en["recent.localSync"]);
  expect(empty).toContain(MESSAGES.en["nav.signIn"]);

  const populated = renderRecent([item], true);
  expect(populated).toContain(MESSAGES.en["recent.localSync"]);
  expect(populated).toContain(MESSAGES.en["nav.signIn"]);
  expect(populated).toContain("English title");
});

test("signed-in recent views omit local-storage sync prompt", () => {
  const empty = renderRecent([], false);
  expect(empty).toContain(MESSAGES.en["recent.empty"]);
  expect(empty).not.toContain(MESSAGES.en["recent.localSync"]);

  const populated = renderRecent([item], false);
  expect(populated).not.toContain(MESSAGES.en["recent.localSync"]);
  expect(populated).toContain("English title");
});

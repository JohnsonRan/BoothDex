import { expect, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import type { ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { Providers } from "@/components/providers";
import type { SemanticStatus } from "@/lib/api-types";
import { browserMessages } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { SearchTrace } from "./search-trace";

// the popover's content, which the server does not render until the trigger opens it
let content: ReactElement | undefined;
function Grab({ semantic }: { semantic: SemanticStatus }) {
  const trace = SearchTrace({
    count: "1 item",
    semantic,
    steps: [],
    serverMs: 1,
    totalMs: 2,
  }) as ReactElement<{ content: ReactElement }>;
  content = trace.props.content;
  return null;
}

const traced = (semantic: SemanticStatus) => {
  renderToStaticMarkup(
    <AppRouterContext value={{} as AppRouterInstance}>
      <Providers settings={DEFAULT_SETTINGS} messages={browserMessages("en")}>
        <Grab semantic={semantic} />
      </Providers>
    </AppRouterContext>,
  );
  return renderToStaticMarkup(content);
};

test("a search still waiting for its meaning-based matches is traced as answered by words alone", () => {
  expect(traced("pending")).toContain(
    browserMessages("en")["trace.mode.unavailable"],
  );
});

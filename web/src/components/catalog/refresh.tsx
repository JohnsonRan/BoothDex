"use client";
// Refreshing results after a filter, sort or page change. The current results stay on screen while the next
// ones load; only a wait of 150 ms or more shows anything (lib/use-delayed): a small spinner beside the count,
// held for at least 300 ms, and the new count and results then fade in. A failed refresh keeps the results with
// an inline Banner to retry (doona apps/lite UI rules, Phase 1d of the rewrite).

import { Button } from "@astryxdesign/core/Button";
import { Spinner } from "@astryxdesign/core/Spinner";
import { VStack } from "@astryxdesign/core/Stack";
import dynamic from "next/dynamic";
import { useRouter } from "next/navigation";
import {
  type ReactNode,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useAccount } from "@/components/account/session";
import { useUi } from "@/components/providers";
import { useApiRecovery } from "@/lib/client-data";
import { cancelOnReducedMotion, swapIn } from "@/lib/motion";
import { useRefresh } from "./filter-url";
import { FailedResults } from "./result-states";

// shown only after a refresh fails, so its code loads then
const Banner = dynamic(
  () => import("@astryxdesign/core/Banner").then((m) => m.Banner),
  { ssr: false },
);

/**
 * Beside the result count: its slot is always laid out, so nothing moves; the spinner is there only while a
 * refresh has been waiting (data-refresh), so nothing animates at rest.
 */
export function RefreshIndicator() {
  const { t } = useUi();
  const { shown } = useRefresh();
  return (
    <VStack
      data-refresh={shown ? "shown" : "idle"}
      role="status"
      aria-label={shown ? t("list.loading") : undefined}
      width={16}
      height={16}
      hAlign="center"
      vAlign="center"
    >
      {shown && <Spinner size="sm" shade="subtle" />}
    </VStack>
  );
}

/**
 * Content a refresh replaces (the count, the results). When the replacement comes after the indicator showed,
 * it fades in; a quick one replaces the old content as is, so nothing appears to happen but the change itself.
 */
export function Swap({
  inline = false,
  children,
}: {
  inline?: boolean;
  children: ReactNode;
}) {
  const { shown } = useRefresh();
  const ref = useRef<HTMLDivElement>(null);
  const armed = useRef(false);
  // armed once the indicator has been on screen (two frames): content that arrives with it does not fade
  useEffect(() => {
    if (!shown) return;
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => {
        armed.current = true;
      });
    });
    return () => cancelAnimationFrame(frame);
  }, [shown]);
  // keyed on the content: a new server render brings new children
  // biome-ignore lint/correctness/useExhaustiveDependencies: children is the trigger, not an input
  useLayoutEffect(() => {
    if (!armed.current || !ref.current) return;
    armed.current = false;
    return cancelOnReducedMotion(swapIn(ref.current));
  }, [children]);
  return (
    <VStack
      ref={ref}
      style={inline ? { display: "inline-flex", minWidth: "6em" } : undefined}
    >
      {children}
    </VStack>
  );
}

/**
 * The results region. `children` are the newest results; a failed load (`failure`) shows the last results
 * this page had with a Banner above them, or the full error state when there were none.
 */
export function KeepResults({
  failure,
  children,
}: {
  failure?: "error" | "starting";
  children?: ReactNode;
}) {
  const [kept, setKept] = useState(children);
  if (!failure && children !== kept) setKept(children);
  if (!failure) return children;
  if (!kept) return <FailedResults starting={failure === "starting"} />;
  return (
    <VStack gap={4}>
      <RefreshFailed starting={failure === "starting"} />
      {kept}
    </VStack>
  );
}

function RefreshFailed({ starting }: { starting: boolean }) {
  const { t } = useUi();
  const router = useRouter();
  useApiRecovery(starting, useAccount().reload);
  return (
    <Banner
      status={starting ? "info" : "error"}
      title={starting ? t("list.startingTitle") : t("list.loadFailed")}
      description={starting ? t("list.starting") : undefined}
      icon={starting ? <Spinner size="sm" /> : undefined}
      endContent={
        !starting && (
          <Button
            label={t("list.retry")}
            variant="secondary"
            size="sm"
            onClick={() => router.refresh()}
          />
        )
      }
    />
  );
}

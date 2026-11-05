"use client";
// Refreshing results after a filter, sort or page change. The current results stay on screen while the next
// ones load, a progress bar under the header says so, and a failed refresh keeps them with an inline Banner
// to retry (doona apps/lite UI rules, Phase 1d of the rewrite).

import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { ProgressBar } from "@astryxdesign/core/ProgressBar";
import { Spinner } from "@astryxdesign/core/Spinner";
import { VStack } from "@astryxdesign/core/Stack";
import { useRouter } from "next/navigation";
import { type ReactNode, useState } from "react";
import { useUi } from "@/components/providers";
import { useApiRecovery } from "@/lib/client-data";
import { useFilterUrl } from "./filter-url";
import { FailedResults } from "./result-states";

/** Under the page header; zero height, so showing it moves nothing. */
export function RefreshProgress() {
  const { t } = useUi();
  const { pending } = useFilterUrl();
  return (
    <VStack
      aria-hidden={!pending}
      style={{
        height: 0,
        position: "relative",
        zIndex: 1,
        visibility: pending ? "visible" : "hidden",
      }}
    >
      <ProgressBar
        label={t("list.loading")}
        isLabelHidden
        isIndeterminate={pending}
        value={0}
      />
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
  useApiRecovery(starting);
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

// A job's status as an Astryx Token, the way the template incident-console labels a row's state
// (templates/pages/incident-console/page.tsx:364-368). Colours: running blue, success green, failure red,
// stopping orange, waiting and cancelled grey.

import { Token, type TokenColor } from "@astryxdesign/core/Token";
import { useAdminT } from "./messages";
import type { JobStatus } from "./types";

const COLOR: Record<JobStatus, TokenColor> = {
  queued: "gray",
  running: "blue",
  cancelling: "orange",
  cancelled: "gray",
  succeeded: "green",
  failed: "red",
};

export function JobStatusToken({ status }: { status: JobStatus }) {
  const t = useAdminT();
  return (
    <Token size="sm" color={COLOR[status]} label={t(`status.${status}`)} />
  );
}

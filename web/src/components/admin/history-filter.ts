// The history's URL filters (?kind=&status=). Kind and the job statuses filter the last 100 jobs in the browser;
// status=unresolved asks the API for the runs the overview counts as unresolved failures (admin/trends.rs), so the
// tile that links here and the list it opens read one definition. The API sends the newest 100 of those with
// their total.

import type { AdminT } from "./messages";
import type { Job } from "./types";

export const ALL = "all";
export const UNRESOLVED = "unresolved";

/** The jobs request behind a status filter. */
export const jobsPath = (status: string) =>
  status === UNRESOLVED ? "/api/admin/jobs?unresolved=true" : "/api/admin/jobs";

/** Whether a row passes the filters the browser applies; for "unresolved" the API has already chosen the rows. */
export const matches = (
  job: Pick<Job, "kind" | "status">,
  kind: string,
  status: string,
) =>
  (kind === ALL || job.kind === kind) &&
  (status === ALL || status === UNRESOLVED || job.status === status);

/** The line over the rows: how many of the `loaded` rows the filters show; for "unresolved", how many the API
 *  counted (`total`) when it sent only the newest of them. */
export function countLine(
  t: AdminT,
  shown: number,
  loaded: number,
  unresolved: boolean,
  total = loaded,
) {
  if (shown < loaded) return t("history.shown", { shown, n: loaded });
  if (!unresolved) return t("history.count", { n: loaded });
  return total > loaded
    ? t("history.unresolvedNewest", { shown: loaded, n: total })
    : t("history.unresolvedCount", { n: total });
}

import type { TokenColor } from "@astryxdesign/core/Token";
import type { GuestbookStatus } from "@/lib/api-types";

export type ClosedStatus = Exclude<GuestbookStatus, "open">;

/** A closed message's Token colour, as GitHub marks a closed issue: purple when completed, grey when not planned.
 *  An open message carries no mark. */
export const CLOSED_COLOR: Record<ClosedStatus, TokenColor> = {
  completed: "purple",
  not_planned: "gray",
};

/** A message as its author typed it: line breaks kept, a long unbroken word wrapped. The drawer, the confirm dialog
 *  and the admin console show a message the same way. */
export const MESSAGE_STYLE = {
  whiteSpace: "pre-wrap",
  overflowWrap: "anywhere",
} as const;

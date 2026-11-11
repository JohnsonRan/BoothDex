import { Fragment, type ReactNode } from "react";

/** A message with its {name} placeholders filled by nodes (links); placeholders without a node stay as written. */
export function rich(
  template: string,
  nodes: Record<string, ReactNode>,
): ReactNode[] {
  return template.split(/(\{\w+\})/).map((part, i) => {
    const name = part.slice(1, -1);
    return part.startsWith("{") && name in nodes ? (
      // biome-ignore lint/suspicious/noArrayIndexKey: the parts of one fixed string, never reordered
      <Fragment key={i}>{nodes[name]}</Fragment>
    ) : (
      part
    );
  });
}

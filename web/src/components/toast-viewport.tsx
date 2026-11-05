"use client";
// The toast viewport (components/toasts.tsx), in its own chunk so the page loads the toast code with the first
// toast. Mounted inside Providers, it shares the page's theme and language.

import {
  type ToastOptions,
  ToastViewport,
  useToast,
} from "@astryxdesign/core/Toast";
import { useEffect, useRef } from "react";

type Props = {
  /** toasts raised before this chunk arrived, in order */
  queue: ToastOptions[];
  /** receives the viewport's toast function, used directly from then on */
  onReady: (toast: (options: ToastOptions) => void) => void;
};

export default function ToastLayer(props: Props) {
  return (
    <ToastViewport>
      <Flush {...props} />
    </ToastViewport>
  );
}

function Flush({ queue, onReady }: Props) {
  // the viewport's own, from its context
  const toast = useToast();
  const shown = useRef(0);
  useEffect(() => {
    for (const options of queue.slice(shown.current)) toast(options);
    shown.current = queue.length;
    onReady(toast);
  }, [queue, toast, onReady]);
  return null;
}

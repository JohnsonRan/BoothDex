"use client";
// The privacy policy and the terms in a dialog over whatever opened them (Settings > About, the register form's
// consent line), so nothing is left behind: Astryx block Dialog — Scrolling content
// (@astryxdesign/cli assets/templates/blocks/components/Dialog/DialogScrollingContent.tsx) without its footer, as the texts ask for no
// answer; the "Last updated" line is the header's subtitle (block DialogWithSubtitle). The body is the page's
// (legal-body.tsx), its data fetched from the server (legal-action.ts) once per text and language. The links keep their
// href, so a modified click still opens /privacy or /terms in a new tab; the terms' link to the privacy policy
// switches the dialog instead of leaving.

import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { Spinner } from "@astryxdesign/core/Spinner";
import {
  type MouseEvent,
  type ReactNode,
  Suspense,
  use,
  useState,
} from "react";
import { useUi } from "@/components/providers";
import { DIALOG_MAX_HEIGHT } from "@/lib/dialog-height";
import type { LegalView as Doc, LegalKind } from "@/lib/legal";
import { loadLegal } from "./legal-action";
import { LegalBody } from "./legal-body";

const docs = new Map<string, Promise<Doc>>();
/** One request per text and language; a failed one opens the page instead. */
function load(kind: LegalKind, ui: string): Promise<Doc> {
  const key = `${kind}:${ui}`;
  let doc = docs.get(key);
  if (!doc) {
    doc = loadLegal(kind).catch(() => {
      docs.delete(key);
      location.assign(`/${kind}`);
      return new Promise<never>(() => {});
    });
    docs.set(key, doc);
  }
  return doc;
}

const modified = (e: MouseEvent) =>
  e.metaKey || e.ctrlKey || e.shiftKey || e.altKey;

function Frame({
  title,
  subtitle,
  onClose,
  children,
}: {
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <Layout
      header={
        <DialogHeader
          title={title}
          subtitle={subtitle}
          onOpenChange={onClose}
        />
      }
      content={<LayoutContent>{children}</LayoutContent>}
    />
  );
}

function Loaded({ doc, onClose }: { doc: Promise<Doc>; onClose: () => void }) {
  const data = use(doc);
  return (
    <Frame title={data.title} subtitle={data.updated} onClose={onClose}>
      <LegalBody doc={data} />
    </Frame>
  );
}

/** `open(kind)` is a link's or a button's onClick; render `dialog` beside the surface that has them. */
export function useLegalDialog() {
  const { t, ui } = useUi();
  // a server action updates the router, so it is called from the click, not while rendering
  const [shown, setShown] = useState<{
    kind: LegalKind;
    doc: Promise<Doc>;
    n: number;
  }>();
  const show = (kind: LegalKind) =>
    setShown((s) => ({ kind, doc: load(kind, ui), n: (s?.n ?? 0) + 1 }));
  const [isOpen, setOpen] = useState(false);
  const open = (next: LegalKind) => (e: MouseEvent) => {
    if (modified(e)) return;
    e.preventDefault();
    show(next);
    setOpen(true);
  };
  const close = () => setOpen(false);
  // before the link's own handler (Next's Link navigates unless the click was cancelled)
  const follow = (e: MouseEvent) => {
    const path =
      e.target instanceof Element &&
      e.target.closest("a")?.getAttribute("href");
    if ((path === "/privacy" || path === "/terms") && !modified(e)) {
      e.preventDefault();
      show(path.slice(1) as LegalKind);
    }
  };
  const dialog = shown && (
    <Dialog
      isOpen={isOpen}
      onOpenChange={setOpen}
      width={960}
      maxHeight={DIALOG_MAX_HEIGHT}
      onClickCapture={follow}
    >
      <Suspense
        // each text shown, the same one opened again too, starts at its top (the dialog stays mounted)
        key={shown.n}
        fallback={
          <Frame title={t(`legal.${shown.kind}`)} onClose={close}>
            <Spinner label={t(`legal.${shown.kind}`)} />
          </Frame>
        }
      >
        <Loaded doc={shown.doc} onClose={close} />
      </Suspense>
    </Dialog>
  );
  return { open, dialog };
}

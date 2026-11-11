"use server";
// The privacy policy's or the terms' body for the dialog (legal-dialog.tsx), rendered on the server as the pages
// render it, so the texts and the operator's details stay out of the browser's bundle.

import type { LegalKind } from "@/lib/legal";
import { legalDoc } from "./legal-screen";

export async function loadLegal(kind: LegalKind) {
  if (kind !== "privacy" && kind !== "terms") throw new Error("unknown text");
  return legalDoc(kind);
}

"use server";
// The privacy policy or the terms for the dialog (legal-dialog.tsx), as the data the pages render, so the texts
// stay out of the browser's bundle. Data, not elements: elements would name client components that the calling
// route's client manifest may not list, which only a production build enforces (React error #441).

import type { LegalKind } from "@/lib/legal";
import { legalDoc } from "./legal-screen";

export async function loadLegal(kind: LegalKind) {
  if (kind !== "privacy" && kind !== "terms") throw new Error("unknown text");
  return legalDoc(kind);
}

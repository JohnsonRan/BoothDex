import { Suspense } from "react";
import { LegalScreen, legalMetadata } from "@/components/legal/legal-screen";

export const metadata = legalMetadata("privacy");

// Settings (cookie) are per request, so the page streams in behind one boundary.
export default function Privacy() {
  return (
    <Suspense>
      <LegalScreen kind="privacy" />
    </Suspense>
  );
}

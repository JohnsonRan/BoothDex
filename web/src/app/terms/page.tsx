import { Suspense } from "react";
import { LegalScreen, legalMetadata } from "@/components/legal/legal-screen";

export const metadata = legalMetadata("terms");

// Settings (cookie) are per request, so the page streams in behind one boundary.
export default function Terms() {
  return (
    <Suspense>
      <LegalScreen kind="terms" />
    </Suspense>
  );
}

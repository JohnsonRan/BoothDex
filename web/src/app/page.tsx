import { Suspense } from "react";
import { Browse } from "@/components/catalog/browse";

// The browse UI reads filters from the URL (useSearchParams), so it renders on the client inside Suspense.
export default function Home() {
  return (
    <Suspense>
      <Browse />
    </Suspense>
  );
}

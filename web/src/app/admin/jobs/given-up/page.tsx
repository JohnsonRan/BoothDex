import type { Metadata } from "next";
import { GivenUpPage } from "@/components/admin/given-up";

export const metadata: Metadata = { title: "AI gave up" };

export default function AdminGivenUp() {
  return <GivenUpPage />;
}

import type { Metadata } from "next";
import { Overview } from "@/components/admin/overview";

export const metadata: Metadata = { title: "Overview" };

export default function AdminOverview() {
  return <Overview />;
}

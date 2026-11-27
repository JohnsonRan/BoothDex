import type { Metadata } from "next";
import { History } from "@/components/admin/history";

export const metadata: Metadata = { title: "Job history" };

export default function AdminHistory() {
  return <History />;
}

import type { Metadata } from "next";
import { Tasks } from "@/components/admin/tasks";

export const metadata: Metadata = { title: "Crawler" };

export default function AdminTasks() {
  return <Tasks />;
}

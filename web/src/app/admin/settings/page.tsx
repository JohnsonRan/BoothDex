import type { Metadata } from "next";
import { SettingsPage } from "@/components/admin/settings-page";

export const metadata: Metadata = { title: "Settings" };

export default function AdminSettings() {
  return <SettingsPage />;
}

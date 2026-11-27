import { SettingsPage } from "@/components/admin/settings-page";
import { adminPageMetadata } from "../metadata";

export const generateMetadata = adminPageMetadata("nav.settings");

export default function AdminSettings() {
  return <SettingsPage />;
}

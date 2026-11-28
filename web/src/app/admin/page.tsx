import { Overview } from "@/components/admin/overview";
import { adminPageMetadata } from "./metadata";

export const generateMetadata = adminPageMetadata("nav.overview");

export default function AdminOverview() {
  return <Overview />;
}

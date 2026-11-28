import { History } from "@/components/admin/history";
import { adminPageMetadata } from "../../metadata";

export const generateMetadata = adminPageMetadata("jobs.history");

export default function AdminHistory() {
  return <History />;
}

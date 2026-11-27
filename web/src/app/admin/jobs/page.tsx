import { Tasks } from "@/components/admin/tasks";
import { adminPageMetadata } from "../metadata";

export const generateMetadata = adminPageMetadata("nav.jobs");

export default function AdminTasks() {
  return <Tasks />;
}

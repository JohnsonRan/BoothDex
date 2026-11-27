import { UsersPage } from "@/components/admin/users";
import { adminPageMetadata } from "../metadata";

export const generateMetadata = adminPageMetadata("nav.users");

export default function AdminUsers() {
  return <UsersPage />;
}

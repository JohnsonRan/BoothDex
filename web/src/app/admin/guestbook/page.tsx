import { GuestbookPage } from "@/components/admin/guestbook";
import { adminPageMetadata } from "../metadata";

export const generateMetadata = adminPageMetadata("nav.guestbook");

export default function AdminGuestbook() {
  return <GuestbookPage />;
}

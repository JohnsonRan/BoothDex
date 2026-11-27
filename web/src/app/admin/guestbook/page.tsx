import type { Metadata } from "next";
import { GuestbookPage } from "@/components/admin/guestbook";

export const metadata: Metadata = { title: "Guestbook" };

export default function AdminGuestbook() {
  return <GuestbookPage />;
}

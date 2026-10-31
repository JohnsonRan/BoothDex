import type { Metadata } from "next";
import { AdminConsole } from "./admin-console";

export const metadata: Metadata = {
  title: "管理后台 · BoothDex",
  robots: { index: false },
};

export default function AdminPage() {
  return <AdminConsole />;
}

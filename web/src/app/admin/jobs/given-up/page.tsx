import { GivenUpPage } from "@/components/admin/given-up";
import { adminPageMetadata } from "../../metadata";

export const generateMetadata = adminPageMetadata("jobs.givenUp");

export default function AdminGivenUp() {
  return <GivenUpPage />;
}

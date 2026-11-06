// Hands the home page its query string as sent. Next gives a page decoded `searchParams`, where a literal "+" and
// an encoded space both read as a space, but the catalogue's lists are separated by a literal "+" and a space
// inside a value is "%20" (crates/api/src/params.rs): only the raw string tells the tag "long hair" from the
// tags "long" and "hair".
import { type NextRequest, NextResponse } from "next/server";
import { RAW_QUERY_HEADER } from "@/lib/catalog";

export function proxy(request: NextRequest) {
  const headers = new Headers(request.headers);
  headers.set(RAW_QUERY_HEADER, request.nextUrl.search);
  return NextResponse.next({ request: { headers } });
}

export const config = { matcher: "/" };

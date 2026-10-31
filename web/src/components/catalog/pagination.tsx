import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useSettings } from "@/lib/settings";

/** Pages to show: first, last and two either side of the current one; null marks a gap ("1 … 4 5 6 7 8 … 72"),
 * except a gap of one page, which shows that page instead. */
export function pageList(page: number, total: number): (number | null)[] {
  const keep = (n: number) => n === 1 || n === total || Math.abs(n - page) <= 2;
  const out: (number | null)[] = [];
  for (let n = 1; n <= total; n++) {
    if (keep(n) || (keep(n - 1) && keep(n + 1))) out.push(n);
    else if (out[out.length - 1] !== null) out.push(null);
  }
  return out;
}

export function Pagination({
  page,
  total,
  onPage,
}: {
  page: number;
  total: number;
  onPage: (page: number) => void;
}) {
  const { t } = useSettings();
  if (total <= 1) return null;
  const pages = pageList(page, total);
  return (
    <nav
      aria-label={t("page.nav")}
      className="flex flex-wrap items-center justify-center gap-1 pt-6 pb-2"
    >
      <Button
        variant="outline"
        size="icon-sm"
        disabled={page <= 1}
        onClick={() => onPage(page - 1)}
        aria-label={t("page.prev")}
      >
        <ChevronLeft />
      </Button>
      {pages.map((n, i) =>
        n === null ? (
          <span
            // a gap sits between two fixed numbers, so its neighbour identifies it
            key={`gap-after-${pages[i - 1]}`}
            className="px-1 text-muted-foreground"
            aria-hidden
          >
            …
          </span>
        ) : (
          <Button
            key={n}
            variant={n === page ? "default" : "ghost"}
            size="sm"
            className="min-w-8 tabular-nums"
            aria-current={n === page ? "page" : undefined}
            onClick={() => onPage(n)}
          >
            {n}
          </Button>
        ),
      )}
      <Button
        variant="outline"
        size="icon-sm"
        disabled={page >= total}
        onClick={() => onPage(page + 1)}
        aria-label={t("page.next")}
      >
        <ChevronRight />
      </Button>
    </nav>
  );
}

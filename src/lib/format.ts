const rtf = new Intl.RelativeTimeFormat("fr", { numeric: "auto" });
const day = new Intl.DateTimeFormat("fr", { day: "numeric", month: "short" });
const dayYear = new Intl.DateTimeFormat("fr", { day: "numeric", month: "short", year: "numeric" });

/** "il y a 5 min", "hier", "12 oct." for anything older than a week. */
export function since(iso: string | number | null | undefined, now = Date.now()): string {
  if (!iso) return "";
  const t = typeof iso === "number" ? iso : Date.parse(iso);
  const s = Math.round((t - now) / 1000);
  const abs = Math.abs(s);
  if (abs < 45) return "à l'instant";
  if (abs < 3600) return rtf.format(Math.round(s / 60), "minute");
  if (abs < 86_400) return rtf.format(Math.round(s / 3600), "hour");
  if (abs < 7 * 86_400) return rtf.format(Math.round(s / 86_400), "day");
  return dateLabel(new Date(t).toISOString());
}

/** The same, compact for narrow columns: "5 min", "3 h", "hier", "4 j", "12 oct.". */
export function ago(iso: string | number | null | undefined, now = Date.now()): string {
  if (!iso) return "";
  const t = typeof iso === "number" ? iso : Date.parse(iso);
  const s = Math.max(0, Math.round((now - t) / 1000));
  if (s < 45) return "à l'instant";
  if (s < 3600) return `${Math.round(s / 60)} min`;
  if (s < 86_400) return `${Math.round(s / 3600)} h`;
  const days = Math.round(s / 86_400);
  if (days === 1) return "hier";
  if (days < 7) return `${days} j`;
  return dateLabel(new Date(t).toISOString());
}

/** A calendar date, with the year only when it is not this one. */
export function dateLabel(iso: string | undefined): string {
  if (!iso) return "";
  const d = new Date(iso.length === 10 ? `${iso}T12:00:00` : iso);
  return (d.getFullYear() === new Date().getFullYear() ? day : dayYear).format(d);
}

/** Days from today to a `YYYY-MM-DD` target; negative once passed. */
export function daysUntil(date: string, now = Date.now()): number {
  const target = new Date(`${date}T23:59:59`).getTime();
  return Math.ceil((target - now) / 86_400_000);
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} o`;
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(n < 10 * 1024 ? 1 : 0)} Ko`;
  return `${(n / 1024 ** 2).toFixed(1)} Mo`;
}

/** `/Users/ada/code/game` gives `~/code/game`. */
export function tilde(path: string): string {
  return path.replace(/^\/Users\/[^/]+/, "~");
}

/** `Armures : slot de torse` gives `armures-slot-de-torse`, as the Rust core does for folders. */
export function slugify(text: string, max = 40): string {
  return text
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/œ/g, "oe")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, max)
    .replace(/-$/, "");
}

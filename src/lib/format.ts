export function fmtPct(p: number, digits = 1): string {
  if (!Number.isFinite(p)) return "—";
  return `${(p * 100).toFixed(digits)}%`;
}

export function fmtOdds(o: number): string {
  // 0 is the backend's invalid-probability sentinel (model.rs::prob_to_american_odds).
  if (!Number.isFinite(o) || o === 0) return "—";
  return o > 0 ? `+${o}` : `${o}`;
}

export function fmtRuns(r: number): string {
  if (!Number.isFinite(r)) return "—";
  return r.toFixed(1);
}

export function todayISO(): string {
  // Local date as YYYY-MM-DD. en-CA is the locale that emits ISO 8601 order.
  return new Date().toLocaleDateString("en-CA");
}

export function relativeTime(iso: string): string {
  const then = new Date(iso).getTime();
  const now = Date.now();
  const sec = Math.round((now - then) / 1000);
  if (sec < 1) return "just now";
  if (sec < 60) return `${sec}s ago`;
  if (sec < 3600) return `${Math.round(sec / 60)}m ago`;
  if (sec < 86400) return `${Math.round(sec / 3600)}h ago`;
  return `${Math.round(sec / 86400)}d ago`;
}

export function downloadCSV(filename: string, rows: Record<string, unknown>[]): void {
  if (rows.length === 0) return;
  const headers = Object.keys(rows[0]);
  const escape = (v: unknown): string => {
    let s = v == null ? "" : String(v);
    // CSV injection guard: a leading = + - @ or TAB/CR turns a cell into a
    // spreadsheet formula when opened in Excel/Sheets. Prefix with a single
    // quote so the value renders as text (the quote is hidden by default).
    if (/^[=+\-@\t\r]/.test(s)) s = `'${s}`;
    return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
  };
  const csv = [
    headers.join(","),
    ...rows.map((r) => headers.map((h) => escape(r[h])).join(",")),
  ].join("\n");
  const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

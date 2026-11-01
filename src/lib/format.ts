export function mb(bytes: number): string {
  const m = bytes / (1024 * 1024);
  return m >= 1024 ? `${(m / 1024).toLocaleString("pt-BR", { maximumFractionDigits: 2 })} GB` : `${Math.round(m).toLocaleString("pt-BR")} MB`;
}

export function speed(bytesPerSec: number): string {
  const m = bytesPerSec / (1024 * 1024);
  return `${m.toLocaleString("pt-BR", { maximumFractionDigits: m < 10 ? 1 : 0 })} MB/s`;
}

export function eta(remaining: number, bytesPerSec: number): string {
  if (bytesPerSec <= 0) return "";
  const s = Math.round(remaining / bytesPerSec);
  if (s < 60) return `${s} s`;
  const min = Math.round(s / 60);
  return min < 60 ? `${min} min` : `${Math.floor(min / 60)} h ${min % 60} min`;
}

/** Tempo de mídia: `m:ss` ou `h:mm:ss`. */
export function clock(t: number): string {
  const s = Math.max(0, Math.floor(t));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

/** Duração legível: "45 s", "2 min 4 s", "1 h 3 min". */
export function duration(t: number): string {
  const s = Math.round(t);
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return s % 60 ? `${m} min ${s % 60} s` : `${m} min`;
  return `${Math.floor(m / 60)} h ${m % 60} min`;
}

/** Data/hora local a partir do ISO em UTC do banco. */
export function when(iso: string): string {
  const d = new Date(iso);
  return isNaN(d.getTime()) ? iso : d.toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

export function num(x: number, digits = 1): string {
  return x.toLocaleString("pt-BR", { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

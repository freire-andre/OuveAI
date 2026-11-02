import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Action = "mini" | "record" | "mark";

export interface ShortcutInfo {
  action: Action;
  accel: string;
  error: string | null;
}

export interface Status {
  stealth: boolean;
  stealth_supported: boolean;
  build: number;
  recording: boolean;
  shortcuts: ShortcutInfo[];
  data_dir: string;
  version: string;
}

/** Falhou e vai tentar de novo; status 0 = sem conexão. */
export interface Retry {
  attempt: number;
  max: number;
  wait_secs: number;
  status: number;
}

export type Step =
  | { step: "downloading"; done: number; total: number; bytes_per_sec: number; retry: Retry | null }
  | { step: "verifying"; done: number; total: number }
  | { step: "extracting"; done: number; total: number };

export type ModelState = "absent" | "downloading" | "paused" | "verifying" | "extracting" | "ready" | "corrupt";

export interface ModelView {
  id: string;
  name: string;
  license: string;
  size: number;
  embedded: boolean;
  state: ModelState;
  detail: string | null;
  step: Step | null;
  partial: number;
  error: string | null;
  browser_url: string | null;
  has_mirror: boolean;
  /** Para que serve (modelos opcionais). */
  note: string | null;
}

export interface MachineCheck {
  windows_build: number;
  ram_total_mb: number;
  ram_free_mb: number;
  cores: number;
  avx2: boolean;
  disk_free_mb: number;
  min_build: number;
  min_ram_mb: number;
  min_cores: number;
  model_mb: number;
}

export interface FfmpegView {
  found: { path: string; version: string; origin: "path" | "local" } | null;
  state: "absent" | "downloading" | "paused" | "verifying" | "extracting" | "ready";
  step: Step | null;
  partial: number;
  error: string | null;
  package: { version: string; size: number; license: string; source: string };
}

// --- Importar arquivo (Fase 3) ---

export type Stage =
  | { stage: "converting" }
  | { stage: "analyzing" }
  | { stage: "recognizing"; chunk: number; chunks: number; done: number; total: number }
  | { stage: "saving" };

export type Phase = { kind: "waiting" } | { kind: "waitingcompare" } | { kind: "loading" } | { kind: "working"; stage: Stage };

export interface QueueItem {
  id: number;
  name: string;
  path: string;
}

export interface Finished {
  item: QueueItem;
  session_id: number | null;
  error: string | null;
  warning: string | null;
}

export interface QueueView {
  current: [QueueItem, Phase] | null;
  pending: QueueItem[];
  finished: Finished[];
}

export interface SessionRow {
  id: number;
  title: string;
  kind: "meeting" | "file";
  source_path: string | null;
  started_at: string;
  status: string;
  duration_s: number | null;
  words: number;
}

export interface Word {
  word: string;
  start: number;
  end: number;
}

export interface Suspect {
  idx: number;
  start: number;
  word: string;
  kind: "pt_pt" | "name" | "language";
  hint: string | null;
}

export interface Report {
  load_s: number;
  duration: number;
  convert_s: number;
  transcribe_s: number;
  rtf: number;
  chunks: number;
  forced_cuts: number[];
  words: number;
}

export interface FileDetail {
  session: SessionRow;
  transcript: { source: string; duration: number; words: Word[]; pauses: { start: number; end: number }[] };
  suspects: Suspect[];
  suspects_per_min: number;
  report: Report | null;
  outputs_exist: boolean;
}

/** Erro de `fileEnqueue` quando falta o FFmpeg (a UI oferece o download). */
export const NEED_FFMPEG = "ffmpeg";

// --- Reunião (Fase 4) ---

export type Ch = "EU" | "OUTROS";

export interface ChannelView {
  state: "opening" | "waiting" | "stuck" | "open" | "bt_idle" | "off" | "error" | "app_missing";
  device: string | null;
  error: string | null;
}

export interface SegView {
  id: number;
  ch: Ch;
  start: number;
  end: number;
  text: string;
  echo: boolean;
}

export interface LiveView {
  state: "idle" | "recording" | "paused" | "finishing";
  session_id: number | null;
  model: "loading" | "ready" | "error";
  model_error: string | null;
  eu: ChannelView;
  outros: ChannelView;
  headphones: boolean;
  mic_elsewhere: { device: string; device_id: string; app: string } | null;
  outros_silent: boolean;
  mic_denied: boolean;
  marks: number;
  notice: string | null;
  segments: SegView[];
}

export interface Tick {
  elapsed: number;
  eu: number;
  outros: number;
}

export interface DeviceInfo {
  id: string;
  name: string;
  form: string;
  input: boolean;
  bluetooth: boolean;
  default: boolean;
}

export type MicMode = { mode: "auto" } | { mode: "device"; id: string } | { mode: "off" };
export type OutrosMode = { mode: "auto" } | { mode: "device"; id: string } | { mode: "app"; id: string };
export type Retention = "none" | "7" | "30" | "always";

export interface AudioSetup {
  devices: DeviceInfo[];
  apps: string[];
  mic: MicMode;
  outros: OutrosMode;
  headphones: "auto" | "yes" | "no";
  headphones_auto: boolean | null;
  retention: Retention;
  mic_denied: boolean;
  app_loopback: boolean;
}

export interface TestTick {
  eu: number;
  outros: number;
  eu_open: boolean;
  outros_open: boolean;
}

export interface SegmentRow {
  id: number;
  channel: Ch;
  start_s: number;
  end_s: number;
  text: string;
  flags: number;
}

export interface MarkRow {
  id: number;
  t_s: number;
  kind: "estrela" | "dispositivo" | "pausa";
  note: string | null;
}

export interface MeetingDetail {
  session: SessionRow;
  segments: SegmentRow[];
  marks: MarkRow[];
  audio_dir: string | null;
  audio_expires_at: string | null;
}

export const FLAG_ECHO = 1;

// ---------- comparar com o Whisper (Parte 4) ----------

/** Um trecho em que o Parakeet e o Whisper discordam (`from`/`to`: palavras da sessão). */
export interface Divergence {
  from: number;
  to: number;
  start: number;
  end: number;
  parakeet: string;
  whisper: string;
}

export interface CompareResult {
  divergences: Divergence[];
  /** Trechos `[início, fim]` em que o Whisper falhou (descartados). */
  unreliable: [number, number][];
  compared: number;
  skipped: number;
  whisper_s: number;
  audio_s: number;
}

export type CompareView = { session_id: number } & (
  | { kind: "loading" }
  | { kind: "converting" }
  | { kind: "comparing"; done: number; total: number }
  | { kind: "cancelling" }
  | { kind: "done"; result: CompareResult }
  | { kind: "failed"; error: string }
);

/** Erro de `compareStart` quando o Whisper não está instalado. */
export const NEED_WHISPER = "whisper";
export const WHISPER_ID = "whisper-large-v3-turbo-int8";

/** Estado da comparação + atualizações (evento `compare`). */
export async function watchCompare(cb: (v: CompareView | null) => void): Promise<() => void> {
  cb(await api.compareState());
  return listen<CompareView | null>("compare", (e) => cb(e.payload));
}

// --- Legendas de vídeo editado (Fase 8) ---

export interface EdlCut {
  file: string;
  in: number;
  out: number;
  position: number;
  /** Palavras do bruto que entram no corte (null sem o `.json` do bruto). */
  words: number | null;
}

export interface EdlView {
  path: string;
  cuts: EdlCut[];
  errors: string[];
  warnings: string[];
  gaps: { start: number; end: number }[];
}

export interface EdlGenerated {
  srt: string;
  cues: number;
  words: number;
}

export type EdlCause = { kind: "shift"; by: number } | { kind: "drift"; from: number; to: number } | { kind: "isolated" };

export interface EdlIssue {
  /** Índice do corte (a partir de 0). */
  cut: number;
  start: number;
  end: number;
  words: number;
  worst: number;
  cause: EdlCause;
  why: string;
}

export interface EdlFix {
  /** Número do corte (a partir de 1). */
  cut: number;
  field: "in" | "out" | "position";
  old: number;
  new: number;
}

export interface EdlCheck {
  matched: number;
  unmatched_subs: number;
  unmatched_check: number;
  median: number;
  max: number;
  issues: EdlIssue[];
  fixes: EdlFix[];
}

export type EdlCheckView = { edl: string; video: string } & (
  | { kind: "loading" }
  | { kind: "working"; stage: Stage }
  | { kind: "done"; check: EdlCheck; generated: EdlGenerated }
  | { kind: "failed"; error: string }
);

/** Estado da verificação + atualizações (evento `edl`). */
export async function watchEdl(cb: (v: EdlCheckView | null) => void): Promise<() => void> {
  cb(await api.edlState());
  return listen<EdlCheckView | null>("edl", (e) => cb(e.payload));
}

/** Resultado da busca. `snippet` traz os termos entre U+0002 e U+0003. */
export interface Hit {
  session_id: number;
  kind: "meeting" | "file";
  title: string;
  started_at: string;
  segment_id: number;
  channel: Ch | "ARQUIVO";
  start_s: number;
  snippet: string;
}

export interface UpdateInfo {
  current: string;
  latest: { version: string; url: string } | null;
  newer: boolean;
}

export interface DataUsage {
  meetings: number;
  files: number;
  audio_bytes: number;
  db_bytes: number;
  models_bytes: number;
}

/** Fuso local em minutos a leste de UTC (Brasília = -180), para a hora do relógio. */
export const tzOffset = () => -new Date().getTimezoneOffset();

/** Quebra o trecho da busca em partes normais e destacadas. */
export function snippetParts(s: string): { text: string; hit: boolean }[] {
  const out: { text: string; hit: boolean }[] = [];
  for (const [i, part] of s.split(/[\u0002\u0003]/).entries()) if (part) out.push({ text: part, hit: i % 2 === 1 });
  return out;
}

export const api = {
  status: () => invoke<Status>("get_status"),
  setStealth: (on: boolean) => invoke("set_stealth", { on }),
  setShortcut: (action: Action, accel: string) => invoke("set_shortcut", { action, accel }),
  resetShortcuts: () => invoke("reset_shortcuts"),
  suspendShortcuts: (on: boolean) => invoke("suspend_shortcuts", { on }),
  toggleRecord: () => invoke("toggle_record"),
  mark: () => invoke("mark"),
  hideMini: () => invoke("hide_mini"),
  openMain: (view?: string) => invoke("open_main", { view }),
  models: () => invoke<ModelView[]>("models_list"),
  compareStart: (id: number) => invoke("compare_start", { id }),
  compareState: () => invoke<CompareView | null>("compare_state"),
  compareCancel: () => invoke("compare_cancel"),
  compareDismiss: () => invoke("compare_dismiss"),
  compareApply: (id: number, chosen: number[]) => invoke<string | null>("compare_apply", { id, chosen }),
  edlOpen: (path: string) => invoke<EdlView>("edl_open", { path }),
  edlGenerate: (path: string) => invoke<EdlGenerated>("edl_generate", { path }),
  edlCheckStart: (path: string, video: string) => invoke("edl_check_start", { path, video }),
  edlState: () => invoke<EdlCheckView | null>("edl_state"),
  edlCancel: () => invoke("edl_cancel"),
  edlDismiss: () => invoke("edl_dismiss"),
  edlApply: (fixes: EdlFix[]) => invoke<string>("edl_apply", { fixes }),
  modelDownload: (id: string, source: "primary" | "mirror") => invoke("model_download", { id, source }),
  modelImport: (id: string, path: string) => invoke("model_import", { id, path }),
  modelPause: (id: string) => invoke("model_pause", { id }),
  modelRemove: (id: string) => invoke("model_remove", { id }),
  ffmpeg: () => invoke<FfmpegView>("ffmpeg_status"),
  ffmpegDownload: () => invoke("ffmpeg_download"),
  ffmpegPause: () => invoke("ffmpeg_pause"),
  fileEnqueue: (paths: string[]) => invoke("file_enqueue", { paths }),
  fileCancel: (id: number) => invoke("file_cancel", { id }),
  fileDismiss: (id: number) => invoke("file_dismiss", { id }),
  fileQueue: () => invoke<QueueView>("file_queue"),
  fileSessions: () => invoke<SessionRow[]>("file_sessions"),
  fileDetail: (id: number) => invoke<FileDetail>("file_detail", { id }),
  fileUpdateWord: (id: number, idx: number, text: string) => invoke<string | null>("file_update_word", { id, idx, text }),
  fileExport: (id: number, dest: string) => invoke("file_export", { id, dest }),
  fileReveal: (id: number) => invoke("file_reveal", { id }),
  fileDelete: (id: number) => invoke("file_delete", { id }),
  machine: () => invoke<MachineCheck>("machine_check"),
  getProxy: () => invoke<string>("get_proxy"),
  setProxy: (proxy: string) => invoke("set_proxy", { proxy }),
  openDataDir: () => invoke("open_data_dir"),
  wizardFinish: () => invoke("wizard_finish"),
  liveState: () => invoke<LiveView>("live_state"),
  livePause: () => invoke("live_pause"),
  liveResume: () => invoke("live_resume"),
  liveUseMic: (deviceId: string) => invoke("live_use_mic", { deviceId }),
  liveRetry: () => invoke("live_retry"),
  liveHeadphones: (on: boolean) => invoke("live_headphones", { on }),
  audioSetup: () => invoke<AudioSetup>("audio_setup"),
  audioSet: (key: "mic" | "outros" | "headphones" | "retention", value: string) => invoke("audio_set", { key, value }),
  audioTestStart: () => invoke("audio_test_start"),
  audioTestStop: () => invoke("audio_test_stop"),
  meetingSessions: () => invoke<SessionRow[]>("meeting_sessions"),
  meetingDetail: (id: number) => invoke<MeetingDetail>("meeting_detail", { id }),
  meetingRename: (id: number, title: string) => invoke("meeting_rename", { id, title }),
  meetingDelete: (id: number) => invoke("meeting_delete", { id }),
  meetingRevealAudio: (id: number) => invoke("meeting_reveal_audio", { id }),
  meetingCopy: (id: number, clock: number | null) => invoke<{ markdown: string; html: string }>("meeting_copy", { id, clock }),
  meetingExport: (id: number, dest: string, clock: number | null) => invoke("meeting_export", { id, dest, clock }),
  meetingClip: (id: number, start: number, end: number) => invoke<ArrayBuffer>("meeting_clip", { id, start, end }),
  search: (q: string) => invoke<Hit[]>("search", { q }),
  dataUsage: () => invoke<DataUsage>("data_usage"),
  deleteAll: () => invoke("delete_all"),
  exportDiagnostics: (dest: string) => invoke("export_diagnostics", { dest }),
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  openRelease: (url: string) => invoke("open_release", { url }),
};

/** Estado da gravação + falas novas e ecos (eventos `live`, `live-segment`, `live-echo`). */
export async function watchLive(cb: (v: LiveView) => void): Promise<() => void> {
  let current = await api.liveState();
  cb(current);
  const offs = await Promise.all([
    listen<LiveView>("live", (e) => cb((current = e.payload))),
    listen<SegView>("live-segment", (e) => {
      const segments = [...current.segments, e.payload].slice(-40);
      cb((current = { ...current, segments }));
    }),
    listen<[number, boolean]>("live-echo", (e) => {
      const [id, echo] = e.payload;
      cb((current = { ...current, segments: current.segments.map((s) => (s.id === id ? { ...s, echo } : s)) }));
    }),
  ]);
  return () => offs.forEach((off) => off());
}

/** dBFS → 0..100 para os medidores (-60 dB = vazio). */
export function meter(db: number): number {
  return Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
}

/** Lista de modelos + atualizações (mudança de estado e progresso). */
export async function watchModels(cb: (m: ModelView[]) => void): Promise<() => void> {
  let current = await api.models();
  cb(current);
  const offs = await Promise.all([
    listen<ModelView[]>("models", (e) => {
      current = e.payload;
      cb(current);
    }),
    listen<[string, Step]>("model-progress", (e) => {
      const [id, step] = e.payload;
      current = current.map((m) => (m.id === id ? { ...m, step } : m));
      cb(current);
    }),
  ]);
  return () => offs.forEach((off) => off());
}

/** Estado do FFmpeg + atualizações (mudança de estado e progresso). */
export async function watchFfmpeg(cb: (f: FfmpegView) => void): Promise<() => void> {
  let current = await api.ffmpeg();
  cb(current);
  const offs = await Promise.all([
    listen<FfmpegView>("ffmpeg", (e) => cb((current = e.payload))),
    listen<Step>("ffmpeg-progress", (e) => cb((current = { ...current, step: e.payload }))),
  ]);
  return () => offs.forEach((off) => off());
}

/** Fila de importação + atualizações (evento `files`). */
export async function watchQueue(cb: (q: QueueView) => void): Promise<() => void> {
  cb(await api.fileQueue());
  return listen<QueueView>("files", (e) => cb(e.payload));
}

/** Status inicial + atualizações empurradas pelo backend (evento `status`). */
export async function watchStatus(cb: (s: Status) => void): Promise<() => void> {
  cb(await api.status());
  return listen<Status>("status", (e) => cb(e.payload));
}

/** Nomes de exibição das teclas modificadoras, no formato aceito pelo backend. */
export function accelFromEvent(e: KeyboardEvent): string | null {
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  let key: string | null = null;
  if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (/^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.code)) key = e.code;
  if (!key || mods.length === 0) return null;
  return [...mods, key].join("+");
}

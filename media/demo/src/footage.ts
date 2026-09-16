/**
 * Actual Codex CLI + Conn recordings. The recording operator supplies the split
 * view; this edit never draws terminal output, tool requests, or approval UI.
 *
 * Capture contract: 1920×1080 (or larger 16:9), Codex CLI left, Conn right.
 * Reserve the top 72px and bottom 80px as neutral #090c12 gutters. The edit puts
 * its heading/caption in those gutters and shows every captured pixel at a
 * constant size. New paths deliberately prevent reuse of the old scripted demo.
 */
export const fps = 30;
export const clipOrder = ["intro", "request", "fix", "reclaim", "resume", "timeline"] as const;
export type ClipId = typeof clipOrder[number];
export type FootageClip = {
  file: string;
  durationInFrames: number;
  trimBeforeSeconds: number;
  playbackRate: number;
};

export const clips: Record<ClipId, FootageClip> = {
  intro: { file: "footage/collaboration/intro.mp4", durationInFrames: 8 * fps, trimBeforeSeconds: 0, playbackRate: 1 },
  request: { file: "footage/collaboration/request.mp4", durationInFrames: 9 * fps, trimBeforeSeconds: 0, playbackRate: 1 },
  fix: { file: "footage/collaboration/fix.mp4", durationInFrames: 11 * fps, trimBeforeSeconds: 0, playbackRate: 1 },
  reclaim: { file: "footage/collaboration/reclaim.mp4", durationInFrames: 11 * fps, trimBeforeSeconds: 0, playbackRate: 1 },
  resume: { file: "footage/collaboration/resume.mp4", durationInFrames: 13 * fps, trimBeforeSeconds: 0, playbackRate: 1 },
  timeline: { file: "footage/collaboration/timeline.mp4", durationInFrames: 6 * fps, trimBeforeSeconds: 0, playbackRate: 1 },
};

export const closingDurationInFrames = 4 * fps;
export const durationInFrames = clipOrder.reduce((total, id) => total + clips[id].durationInFrames, closingDurationInFrames);

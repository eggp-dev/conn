import { Video } from "@remotion/media";
import { AbsoluteFill, CanvasImage, Easing, Sequence, interpolate, staticFile, useCurrentFrame } from "remotion";
import { fontFamily } from "../fonts";
import { camera, clicks, closingSeconds, cues, footageSeconds, remoteCopy, remoteFps, shortClosingSeconds, shortDissolve, shortParts, type RemoteLanguage } from "./edit";

const STAGE = { w: 1920, h: 1080 };
// The take is 1200x540 CSS pixels at 2x. At 1.4x on the stage its 19px terminal type reads at about 27px.
const WINDOW = { w: 1680, h: 756, left: 120, top: 84, source: { w: 1200, h: 540 } };
const ease = Easing.bezier(0.45, 0, 0.15, 1);

function shotAt(second: number) {
  const i = Math.max(1, camera.findIndex((s) => s.t >= second));
  const a = camera[i - 1] ?? camera[0], b = camera[i] ?? camera[camera.length - 1];
  const k = b.t === a.t ? 1 : ease(Math.min(1, Math.max(0, (second - a.t) / (b.t - a.t))));
  return { scale: a.scale + (b.scale - a.scale) * k, x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k };
}

/** The recording as one window on a black stage. The camera moves; the recording itself is never cut. */
const Recording: React.FC<{ offset?: number; intro?: boolean }> = ({ offset = 0, intro = true }) => {
  const local = useCurrentFrame() / remoteFps;
  const second = local + offset;
  const shot = shotAt(second);
  // Keep the chosen point of the recording in the middle of the window, without showing past its edges.
  const clamp = (v: number, size: number) => Math.min(0, Math.max(size - size * shot.scale, size / 2 - v * size * shot.scale));
  const tx = clamp(shot.x, WINDOW.w), ty = clamp(shot.y, WINDOW.h);
  const appear = intro ? interpolate(local, [0, 0.7], [0, 1], { extrapolateRight: "clamp", easing: Easing.out(Easing.cubic) }) : 1;
  return <div style={{ position: "absolute", left: WINDOW.left, top: WINDOW.top, width: WINDOW.w, height: WINDOW.h, borderRadius: 22, overflow: "hidden",
    opacity: appear, scale: String(interpolate(appear, [0, 1], [0.985, 1])), boxShadow: "0 0 0 1px #ffffff17, 0 40px 120px #7d6bff1f, 0 8px 30px #000c", background: "#0b0d14" }}>
    <div style={{ position: "absolute", width: WINDOW.w, height: WINDOW.h, transformOrigin: "0 0", transform: `translate(${tx}px, ${ty}px) scale(${shot.scale})` }}>
      <Video name="One continuous take of Conn with a real agent" src={staticFile("footage/remote/take.mp4")} trimBefore={Math.round(offset * remoteFps)} muted objectFit="fill" style={{ width: WINDOW.w, height: WINDOW.h }} />
      {clicks.map((c) => {
        const age = second - c.t;
        if (age < -0.12 || age > 0.7) return null;
        const grow = interpolate(age, [-0.12, 0.7], [0.2, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.out(Easing.cubic) });
        const size = 96 * grow;
        return <div key={c.t} style={{ position: "absolute", left: (c.x / WINDOW.source.w) * WINDOW.w - size / 2, top: (c.y / WINDOW.source.h) * WINDOW.h - size / 2, width: size, height: size,
          borderRadius: "50%", border: "2.5px solid #ffffff", background: "#ffffff22", opacity: interpolate(age, [-0.12, 0, 0.7], [0, 0.95, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }) }} />;
      })}
    </div>
  </div>;
};

const Caption: React.FC<{ language: RemoteLanguage; offset?: number }> = ({ language, offset = 0 }) => {
  const second = useCurrentFrame() / remoteFps + offset;
  return <>{cues(language).map((cue) => {
    if (second < cue.start - 0.05 || second > cue.end + 0.05) return null;
    const fade = 0.32;
    const opacity = Math.min(interpolate(second, [cue.start, cue.start + fade], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }), interpolate(second, [cue.end - fade, cue.end], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }));
    const rise = interpolate(second, [cue.start, cue.start + 0.5], [10, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.out(Easing.cubic) });
    return <div key={cue.start} style={{ position: "absolute", left: 120, right: 120, top: WINDOW.top + WINDOW.h + 22, height: STAGE.h - WINDOW.top - WINDOW.h - 30, display: "flex", flexDirection: "column",
      alignItems: "center", justifyContent: "center", gap: 8, opacity, translate: `0px ${rise}px`, textAlign: "center" }}>
      {cue.quote && <div style={{ fontSize: 24, color: "#9a9aad", fontWeight: 400, letterSpacing: 0.2 }}>{cue.sub}</div>}
      <div style={{ fontSize: cue.quote ? 38 : language === "ko" ? 50 : 54, fontWeight: cue.quote ? 400 : 700, color: cue.quote ? "#e6e2ff" : "#ffffff", letterSpacing: cue.quote ? -0.3 : -1.4, lineHeight: 1.18 }}>{cue.line}</div>
      {!cue.quote && cue.sub && <div style={{ fontSize: 27, color: "#a3a3b5", fontWeight: 400, letterSpacing: -0.2 }}>{cue.sub}</div>}
    </div>;
  })}</>;
};

const Closing: React.FC<{ language: RemoteLanguage; short?: boolean }> = ({ language, short = false }) => {
  const frame = useCurrentFrame(); const copy = remoteCopy[language];
  const inn = (from: number) => ({ opacity: interpolate(frame, [from, from + 14], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }),
    translate: `0px ${interpolate(frame, [from, from + 20], [14, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.out(Easing.cubic) })}px` });
  return <AbsoluteFill style={{ background: "#000", alignItems: "center", justifyContent: "center", fontFamily }}>
    <div style={{ position: "absolute", width: 900, height: 900, background: "radial-gradient(circle, #8f7dff1c, transparent 66%)" }} />
    <CanvasImage src={staticFile("conn-icon.svg")} style={{ width: 92, height: 92, borderRadius: 22, marginBottom: 44, ...inn(4) }} />
    <div style={{ fontSize: language === "ko" ? 82 : 88, fontWeight: 700, letterSpacing: -2.6, lineHeight: 1.14, color: "#fff", textAlign: "center", ...inn(10) }}>{copy.headline[0]}</div>
    <div style={{ fontSize: language === "ko" ? 82 : 88, fontWeight: 700, letterSpacing: -2.6, lineHeight: 1.14, color: "#b9adff", textAlign: "center", ...inn(short ? 24 : 34) }}>{copy.headline[1]}</div>
    <div style={{ marginTop: 46, fontSize: 30, color: "#a3a3b5", ...inn(short ? 44 : 62) }}>{copy.closing}</div>
    <div style={{ marginTop: 14, fontSize: 32, color: "#d9d3ff", ...inn(short ? 50 : 70) }}>github.com/eggplantiny/conn</div>
    <div style={{ position: "absolute", bottom: 56, left: 160, right: 160, textAlign: "center", fontSize: 21, color: "#6f6f82", lineHeight: 1.4, ...inn(short ? 56 : 84) }}>{copy.disclosure}</div>
  </AbsoluteFill>;
};

export const RemoteFilm: React.FC<{ language: RemoteLanguage }> = ({ language }) => {
  const frame = useCurrentFrame(); const footageFrames = Math.round(footageSeconds * remoteFps);
  const out = interpolate(frame, [footageFrames - 14, footageFrames], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return <AbsoluteFill style={{ background: "#000", fontFamily, color: "#fff" }}>
    <Sequence name="One continuous take" durationInFrames={footageFrames}>
      <AbsoluteFill style={{ opacity: out }}><Recording /><Caption language={language} /></AbsoluteFill>
    </Sequence>
    <Sequence name="Closing" from={footageFrames} durationInFrames={Math.round(closingSeconds * remoteFps)}><Closing language={language} /></Sequence>
  </AbsoluteFill>;
};

/** Fifteen seconds for a README or a feed: the login, the refusal and the takeover, then the line. */
export const RemoteShort: React.FC<{ language: RemoteLanguage }> = ({ language }) => {
  const frame = useCurrentFrame(); const d = Math.round(shortDissolve * remoteFps);
  let from = 0;
  const parts = shortParts.map((part, i) => { const length = Math.round((part.to - part.from) * remoteFps); const at = from; from += length; return { ...part, at, length, i }; });
  return <AbsoluteFill style={{ background: "#000", fontFamily, color: "#fff" }}>
    {parts.map((part) => {
      const local = frame - part.at;
      const opacity = Math.min(interpolate(local, [0, d], [part.i === 0 ? 1 : 0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }), interpolate(local, [part.length - d, part.length], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }));
      return <Sequence key={part.i} name={`Moment ${part.i + 1}`} from={part.at} durationInFrames={part.length}>
        <AbsoluteFill style={{ opacity }}><Recording offset={part.from} intro={part.i === 0} /><Caption language={language} offset={part.from} /></AbsoluteFill>
      </Sequence>;
    })}
    <Sequence name="Closing" from={from} durationInFrames={Math.round(shortClosingSeconds * remoteFps)}><Closing language={language} short /></Sequence>
  </AbsoluteFill>;
};

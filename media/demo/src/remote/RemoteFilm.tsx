import { Video } from "@remotion/media";
import { AbsoluteFill, CanvasImage, Easing, Freeze, Sequence, interpolate, staticFile, useCurrentFrame } from "remotion";
import { fontFamily } from "../fonts";
import { camera, clicks, closingSeconds, cues, footageSeconds, remoteCopy, remoteFps, replySeconds, shortClosingSeconds, shortDissolve, shortParts, type Cue, type RemoteLanguage } from "./edit";
import { worksWith } from "../brand";
import { repository } from "../brand";

const STAGE = { w: 1920, h: 1080 };
// The take is 1200x540 CSS pixels at 2x. At 1.4x on the stage its 19px terminal type reads at about 27px.
const WINDOW = { w: 1680, h: 756, left: 120, top: 60, source: { w: 1200, h: 540 } };
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

/** Inline code in a message: `like this`. */
const Rich: React.FC<{ text: string }> = ({ text }) => <>{text.split("`").map((part, i) => i % 2
  ? <span key={i} style={{ fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace", fontSize: "0.86em", padding: "2px 9px", borderRadius: 9, background: "#ffffff14", whiteSpace: "nowrap" }}>{part}</span>
  : <span key={i}>{part}</span>)}</>;

/** A message between you and your agent. The agent lives in another app; this card is how the film says so. */
const MessageCard: React.FC<{ cue: Cue; language: RemoteLanguage; age: number }> = ({ cue, language, age }) => {
  const you = cue.from === "you"; const copy = remoteCopy[language];
  // You type; the agent thinks for a beat, then answers.
  const typed = you ? Math.floor(interpolate(age, [0.25, 1.55], [0, cue.line.length], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })) : cue.line.length;
  const thinking = !you && age < 0.75;
  return <div style={{ display: "flex", flexDirection: "column", alignItems: you ? "flex-end" : "flex-start", gap: 12, width: 1240 }}>
    <div style={{ display: "flex", alignItems: "center", gap: 12, fontSize: 23, color: "#9c9cae" }}>
      <div style={{ width: 34, height: 34, borderRadius: 10, display: "flex", alignItems: "center", justifyContent: "center", fontSize: 17, fontWeight: 700,
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace", background: you ? "#7767ff" : "#23232d", color: you ? "#fff" : "#d9d5ff", boxShadow: you ? "none" : "inset 0 0 0 1px #ffffff1f" }}>{you ? "↑" : ">_"}</div>
      <span style={{ color: "#e4e2f2", fontWeight: 700 }}>{you ? `${cue.sub} → Claude Code` : cue.sub}</span>
      <span>{copy.agentNote}</span>
    </div>
    <div style={{ maxWidth: 1240, padding: "18px 30px", borderRadius: 30, fontSize: language === "ko" ? 31 : 32, lineHeight: 1.34, letterSpacing: -0.4, textAlign: "left",
      background: you ? "linear-gradient(180deg, #8575ff, #6b5bf3)" : "#1c1c25", color: "#fff", boxShadow: you ? "0 18px 50px #6b5bf340" : "inset 0 0 0 1px #ffffff1a, 0 18px 50px #0009",
      borderBottomRightRadius: you ? 10 : 30, borderBottomLeftRadius: you ? 30 : 10, minHeight: 46 }}>
      {thinking ? <span style={{ display: "inline-flex", gap: 9, padding: "10px 4px" }}>{[0, 1, 2].map((i) => <span key={i} style={{ width: 12, height: 12, borderRadius: 6, background: "#a7a3c4",
        opacity: interpolate((age * 3 + i * 0.33) % 1, [0, 0.5, 1], [0.3, 1, 0.3]) }} />)}</span> : <Rich text={cue.line.slice(0, typed)} />}
    </div>
  </div>;
};

const Caption: React.FC<{ language: RemoteLanguage; offset?: number }> = ({ language, offset = 0 }) => {
  const second = useCurrentFrame() / remoteFps + offset;
  return <>{cues(language).map((cue) => {
    if (second < cue.start - 0.05 || second > cue.end + 0.05) return null;
    const fade = 0.32;
    const opacity = Math.min(interpolate(second, [cue.start, cue.start + fade], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }), interpolate(second, [cue.end - fade, cue.end], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }));
    const rise = interpolate(second, [cue.start, cue.start + 0.5], [12, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp", easing: Easing.out(Easing.cubic) });
    return <div key={cue.start} style={{ position: "absolute", left: 120, right: 120, top: WINDOW.top + WINDOW.h + 18, height: STAGE.h - WINDOW.top - WINDOW.h - 26, display: "flex", flexDirection: "column",
      alignItems: "center", justifyContent: "center", gap: 8, opacity, translate: `0px ${rise}px`, textAlign: "center" }}>
      {cue.from ? <MessageCard cue={cue} language={language} age={second - cue.start} /> : <>
        <div style={{ fontSize: language === "ko" ? 50 : 54, fontWeight: 700, color: "#ffffff", letterSpacing: -1.4, lineHeight: 1.18 }}>{cue.line}</div>
        {cue.sub && <div style={{ fontSize: 27, color: "#a3a3b5", fontWeight: 400, letterSpacing: -0.2 }}>{cue.sub}</div>}
      </>}
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
    <div style={{ marginTop: 14, fontSize: 32, color: "#d9d3ff", ...inn(short ? 50 : 70) }}>{repository}</div>
    <div style={{ marginTop: 40, display: "flex", alignItems: "center", gap: 14, fontSize: 24, color: "#8c8c9f", ...inn(short ? 56 : 82) }}>
      <span>{copy.worksWith}</span>
      {worksWith.map((name) => <span key={name} style={{ padding: "7px 18px", borderRadius: 999, color: "#dcd9ee", background: "#ffffff0d", boxShadow: "inset 0 0 0 1px #ffffff1c" }}>{name}</span>)}
    </div>
    <div style={{ position: "absolute", bottom: 56, left: 160, right: 160, textAlign: "center", fontSize: 21, color: "#6f6f82", lineHeight: 1.4, ...inn(short ? 62 : 96) }}>{copy.disclosure}</div>
  </AbsoluteFill>;
};

export const RemoteFilm: React.FC<{ language: RemoteLanguage }> = ({ language }) => {
  const frame = useCurrentFrame(); const footageFrames = Math.round(footageSeconds * remoteFps); const replyFrames = Math.round(replySeconds * remoteFps);
  const bodyFrames = footageFrames + replyFrames;
  const out = interpolate(frame, [bodyFrames - 14, bodyFrames], [1, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  return <AbsoluteFill style={{ background: "#000", fontFamily, color: "#fff" }}>
    <Sequence name="One continuous take" durationInFrames={footageFrames}><AbsoluteFill><Recording /></AbsoluteFill></Sequence>
    {/* The take has ended on a still screen; keep showing its last frame while the agent's reply is read. */}
    <Sequence name="Last frame of the take, held" from={footageFrames} durationInFrames={replyFrames}>
      <AbsoluteFill style={{ opacity: out }}><Freeze frame={footageFrames - 2}><Recording intro={false} /></Freeze></AbsoluteFill>
    </Sequence>
    <Sequence name="Captions and messages" durationInFrames={bodyFrames}><AbsoluteFill style={{ opacity: out }}><Caption language={language} /></AbsoluteFill></Sequence>
    <Sequence name="Closing" from={bodyFrames} durationInFrames={Math.round(closingSeconds * remoteFps)}><Closing language={language} /></Sequence>
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

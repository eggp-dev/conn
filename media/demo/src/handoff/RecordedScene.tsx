import { Video } from "@remotion/media";
import { AbsoluteFill, Interactive, staticFile, useCurrentFrame } from "remotion";
import { BrandMark } from "../BrandMark";
import { Stage } from "../Stage";
import { handoffFps, type CaptionCue } from "./edit";

/** Full-frame recording at a fixed scale; source gutters keep all UI visible. */
export const RecordedScene: React.FC<{
  file: string;
  trim?: number;
  cues: CaptionCue[];
  offset?: number;
}> = ({ file, trim = 0, cues, offset = 0 }) => {
  const frame = useCurrentFrame();
  const second = frame / handoffFps + offset;
  const cue = cues.find((item) => second >= item.start && second < item.end);

  return <Stage>
    <AbsoluteFill>
      <Video name="Actual Codex CLI and Conn recording" src={staticFile(file)}
        trimBefore={Math.round(trim * handoffFps)} muted objectFit="contain"
        style={{ width: "100%", height: "100%" }} />
    </AbsoluteFill>
    <div style={{ position: "absolute", left: 38, right: 38, top: 15, height: 44,
      display: "flex", alignItems: "center", gap: 14 }}>
      <BrandMark size={33} color="#c6bef5" animated={false} />
      <Interactive.Div name="Editorial headline" style={{ fontSize: 34, fontWeight: 700,
        lineHeight: 1.16, letterSpacing: -0.5 }}>{cue?.title}</Interactive.Div>
    </div>
    <Interactive.Div name="Editorial subtitle in recording gutter" style={{
      position: "absolute", left: 60, right: 60, bottom: 24, height: 42,
      display: "flex", alignItems: "center", justifyContent: "center",
      fontSize: 30, color: "#d0d1dc", lineHeight: 1.35, textAlign: "center",
    }}>{cue?.caption}</Interactive.Div>
  </Stage>;
};

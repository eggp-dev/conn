import { Video } from "@remotion/media";
import { Interactive, staticFile, useVideoConfig } from "remotion";
import { BrandMark } from "./BrandMark";
import { copy, type Language } from "./copy";
import { clips, type ClipId } from "./footage";
import { Stage } from "./Stage";

/** A fixed frame: motion belongs to the two real apps, not an editorial camera. */
export const FootageStage: React.FC<{ id: ClipId; language: Language; accent: string }> = ({ id, language, accent }) => {
  const { fps } = useVideoConfig();
  const clip = clips[id];
  const text = copy[language][id];

  return <Stage>
    <Interactive.Div name="Actual Codex CLI and Conn recording" style={{
      position: "absolute", left: 48, top: 27, width: 1824, height: 1026,
      overflow: "hidden", background: "#090c12", borderRadius: 14,
      border: "1px solid #ffffff15",
    }}>
      <Video name={`${id}: actual split-view screencast`} src={staticFile(clip.file)} muted objectFit="contain"
        trimBefore={Math.round(clip.trimBeforeSeconds * fps)} playbackRate={clip.playbackRate}
        style={{ width: "100%", height: "100%" }} />
    </Interactive.Div>
    <div style={{ position: "absolute", left: 76, top: 45, height: 46, display: "flex", alignItems: "center", gap: 16 }}>
      <BrandMark size={30} color={accent} animated={false} />
      <Interactive.Div name="Scene headline" style={{ fontSize: 38, fontWeight: 700, lineHeight: 1.15, letterSpacing: -0.65 }}>
        {text.title}
      </Interactive.Div>
    </div>
    <Interactive.Div name="Scene caption" style={{
      position: "absolute", left: 84, right: 84, top: 996,
      textAlign: "center", fontSize: 32,
      color: "#c2c6d2", lineHeight: 1.3, fontWeight: 400,
    }}>
      {text.caption}
    </Interactive.Div>
  </Stage>;
};

import { CanvasImage, staticFile, useVideoConfig } from "remotion";
import { BrandMark } from "../BrandMark";
import { Stage } from "../Stage";
import { handoffCopy, type HandoffLanguage } from "./edit";
import { repository } from "../brand";

/** The product screenshot is a frame from the real recording, never a mockup. */
export const HandoffPoster: React.FC<{ language: HandoffLanguage }> = ({ language }) => {
  const { width } = useVideoConfig();
  const ratio = width / 1920;
  const copy = handoffCopy[language];
  return <Stage>
    <div style={{ position: "absolute", width: 1920, height: 1080, left: 0, top: 0,
      scale: ratio, transformOrigin: "top left" }}>
      <div style={{ position: "absolute", left: 980, top: -130, width: 950, height: 950,
        background: "radial-gradient(circle, #9980ed14, transparent 67%)" }} />
      <div style={{ position: "absolute", left: 82, top: 84, display: "flex", gap: 16, alignItems: "center",
        fontSize: 32, fontWeight: 700, letterSpacing: 4, color: "#e7e2f9" }}>
        <BrandMark size={51} animated={false} /> CONN
      </div>
      <div style={{ position: "absolute", left: 81, top: 260, width: 638, fontSize: language === "ko" ? 70 : 72,
        fontWeight: 700, lineHeight: 1.24, letterSpacing: -2.8 }}>
        {copy.poster.map((line) => <div key={line}>{line}</div>)}
      </div>
      <div style={{ position: "absolute", left: 84, top: 622, width: 572, fontSize: 30,
        color: "#b6b7c6", lineHeight: 1.5, whiteSpace: "pre-line", wordBreak: "keep-all" }}>{copy.posterDetail}</div>
      <div style={{ position: "absolute", left: 746, top: 255, width: 1098, height: 618, overflow: "hidden",
        borderRadius: 16, border: "1px solid #a99fca38", boxShadow: "0 30px 75px #00000070" }}>
        <CanvasImage src={staticFile("footage/handoff/poster.png")}
          style={{ width: "100%", height: "100%", objectFit: "contain" }} />
      </div>
      <div style={{ position: "absolute", top: 890, left: 747, color: "#a4a1b8", fontSize: 23 }}>
        Codex CLI + Conn · {language === "ko" ? "실제 녹화" : "Actual recording"}
      </div>
      <div style={{ position: "absolute", left: 84, top: 830, display: "flex", gap: 15, alignItems: "center",
        fontSize: 30, color: "#ddd3ff" }}>
        <span style={{ width: 48, height: 48, borderRadius: "50%", border: "1px solid #bba6fa60",
          display: "flex", alignItems: "center", justifyContent: "center" }}>
          <svg width="15" height="18" viewBox="0 0 15 18"><path d="M1 1L14 9L1 17Z" fill="#c7b7ff" /></svg>
        </span>{copy.watch}
      </div>
      <div style={{ position: "absolute", left: 85, bottom: 82, fontSize: 26, color: "#88879d" }}>{repository}</div>
    </div>
  </Stage>;
};

import { CanvasImage, Easing, Interactive, interpolate, staticFile, useCurrentFrame } from "remotion";
import { Stage } from "../Stage";
import { handoffCopy, type HandoffLanguage } from "./edit";
import { repository } from "../brand";

export const ClosingScene: React.FC<{ language: HandoffLanguage; short?: boolean }> = ({ language, short = false }) => {
  const frame = useCurrentFrame();
  const copy = handoffCopy[language];
  return <Stage>
    <div style={{ position: "absolute", left: 615, top: 10, width: 690, height: 690,
      background: "radial-gradient(circle, #b6a2ff16, transparent 69%)" }} />
    <CanvasImage src={staticFile("conn-icon.svg")} style={{ position: "absolute", top: 148, left: 907,
      width: 106, height: 106, borderRadius: 25 }} />
    <Interactive.Div name="Keep your agent. Share your terminal." style={{
      position: "absolute", top: 326, left: 130, right: 130, fontSize: language === "ko" ? 91 : 100,
      fontWeight: 700, lineHeight: 1.16, letterSpacing: -3, textAlign: "center",
      opacity: interpolate(frame, [0, short ? 6 : 12], [0, 1], { extrapolateRight: "clamp" }),
      translate: interpolate(frame, [0, short ? 6 : 16], ["0px 10px", "0px 0px"], {
        extrapolateRight: "clamp", easing: Easing.bezier(0.16, 1, 0.3, 1),
      }),
    }}>{copy.headline[0]}<br />{copy.headline[1]}</Interactive.Div>
    <Interactive.Div name="Download Conn" style={{ position: "absolute", top: 631, left: 120, right: 120,
      textAlign: "center", fontSize: 31, color: "#b5b4c6" }}>{copy.closing}</Interactive.Div>
    <Interactive.Div name="Repository URL" style={{ position: "absolute", top: 714, left: 120, right: 120,
      textAlign: "center", fontSize: 36, color: "#c5bafa" }}>{repository}</Interactive.Div>
    <Interactive.Div name="Recording disclosure" style={{ position: "absolute", bottom: 86, left: 100, right: 100,
      textAlign: "center", fontSize: 24, color: "#9999ac" }}>{copy.disclosure}</Interactive.Div>
  </Stage>;
};

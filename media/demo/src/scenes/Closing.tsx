import { CanvasImage, Interactive, interpolate, staticFile, useCurrentFrame } from "remotion";
import { closingCopy, type Language } from "../copy";
import { Stage } from "../Stage";
import { repository } from "../brand";

export const Closing: React.FC<{ language: Language }> = ({ language }) => {
  const frame = useCurrentFrame();
  return <Stage>
    <div style={{ position: "absolute", width: 550, height: 550, borderRadius: "50%", left: 685, top: 175,
      background: "radial-gradient(circle, #8774df20, #080a1000 67%)" }} />
    <CanvasImage src={staticFile("conn-icon.svg")} style={{ position: "absolute", width: 122, height: 122,
      left: 899, top: 227, borderRadius: 28,
      opacity: interpolate(frame, [0, 12], [0, 1], { extrapolateRight: "clamp" }) }} />
    <Interactive.Div name="Closing message" style={{ position: "absolute", top: 401, left: 120, right: 120, textAlign: "center",
      fontSize: language === "ko" ? 84 : 90, fontWeight: 700, letterSpacing: -3.2, lineHeight: 1.18,
      opacity: interpolate(frame, [6, 22], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }) }}>
      {language === "ko" ? <>쓰던 에이전트와,<br />하나의 터미널에서.</> : <>Your agent.<br />One shared terminal.</>}
    </Interactive.Div>
    <Interactive.Div name="Repository" style={{ position: "absolute", top: 683, left: 100, right: 100, textAlign: "center",
      fontSize: 32, color: "#beb4f7", letterSpacing: 0.5 }}>
      {repository}
    </Interactive.Div>
    <Interactive.Div name="Recording disclosure" style={{ position: "absolute", bottom: 93, left: 100, right: 100, textAlign: "center",
      fontSize: 22, color: "#9699aa" }}>
      {closingCopy[language].disclosure}
    </Interactive.Div>
  </Stage>;
};

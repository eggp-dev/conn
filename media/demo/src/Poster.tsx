import { CanvasImage, staticFile } from "remotion";
import { Stage } from "./Stage";
import { repository } from "./brand";

export const Poster: React.FC = () => <Stage>
  <div style={{ position: "absolute", width: 880, height: 880, left: 900, top: 30, borderRadius: "50%",
    background: "radial-gradient(circle, #9a89ef1c, transparent 66%)" }} />
  <CanvasImage src={staticFile("conn-icon.svg")} style={{ position: "absolute", width: 270, height: 270, right: 225, top: 335,
    borderRadius: 64, boxShadow: "0 30px 90px #00000075" }} />
  <div style={{ position: "absolute", top: 238, left: 170, fontSize: 27, fontWeight: 700, color: "#b5aaff", letterSpacing: 5 }}>CONN</div>
  <div style={{ position: "absolute", top: 321, left: 164, fontSize: 94, lineHeight: 1.13, letterSpacing: -4, fontWeight: 700 }}>
    Your agent.<br />One shared terminal.
  </div>
  <div style={{ position: "absolute", top: 607, left: 172, fontSize: 32, color: "#b9bbcc" }}>
    Take a turn. Keep working together.
  </div>
  <div style={{ position: "absolute", top: 760, left: 172, display: "flex", alignItems: "center", gap: 18, color: "#e1dafa", fontSize: 28 }}>
    <div style={{ width: 55, height: 55, borderRadius: "50%", border: "1px solid #b5aaff55", display: "flex", alignItems: "center", justifyContent: "center" }}>
      <svg width="17" height="19" viewBox="0 0 17 19"><path d="M2 1L16 9.5L2 18Z" fill="#c9bfff" /></svg>
    </div>
    Watch Codex + Conn work together
  </div>
  <div style={{ position: "absolute", left: 172, bottom: 115, fontSize: 24, color: "#717689" }}>{repository}</div>
</Stage>;

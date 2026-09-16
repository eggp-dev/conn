import "./index.css";
import { Composition, Still } from "remotion";
import { Demo } from "./Demo";
import { Poster } from "./Poster";
import { durationInFrames, fps } from "./footage";

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition id="ConnDemoEN" component={Demo} durationInFrames={durationInFrames} fps={fps} width={1920} height={1080} defaultProps={{ language: "en" }} />
      <Composition id="ConnDemoKO" component={Demo} durationInFrames={durationInFrames} fps={fps} width={1920} height={1080} defaultProps={{ language: "ko" }} />
      <Still id="ConnPoster" component={Poster} width={1920} height={1080} />
    </>
  );
};

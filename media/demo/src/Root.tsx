import "./index.css";
import { Composition, Still } from "remotion";
import { Demo } from "./Demo";
import { Poster } from "./Poster";
import { durationInFrames, fps } from "./footage";
import { HandoffFilm, HandoffShort } from "./handoff/HandoffFilm";
import { HandoffPoster } from "./handoff/HandoffPoster";
import { handoffDuration, handoffFps, shortDuration } from "./handoff/edit";

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition id="ConnHandoffEN" component={HandoffFilm} durationInFrames={handoffDuration} fps={handoffFps} width={1920} height={1080} defaultProps={{ language: "en" }} />
      <Composition id="ConnHandoffKO" component={HandoffFilm} durationInFrames={handoffDuration} fps={handoffFps} width={1920} height={1080} defaultProps={{ language: "ko" }} />
      <Composition id="ConnHandoffShortEN" component={HandoffShort} durationInFrames={shortDuration} fps={handoffFps} width={1920} height={1080} defaultProps={{ language: "en" }} />
      <Composition id="ConnHandoffShortKO" component={HandoffShort} durationInFrames={shortDuration} fps={handoffFps} width={1920} height={1080} defaultProps={{ language: "ko" }} />
      <Still id="ConnHandoffPosterEN" component={HandoffPoster} width={1920} height={1080} defaultProps={{ language: "en" }} />
      <Still id="ConnHandoffPosterKO" component={HandoffPoster} width={1920} height={1080} defaultProps={{ language: "ko" }} />
      <Still id="ConnHandoffSocialEN" component={HandoffPoster} width={1280} height={720} defaultProps={{ language: "en" }} />
      <Still id="ConnHandoffSocialKO" component={HandoffPoster} width={1280} height={720} defaultProps={{ language: "ko" }} />
      <Composition id="ConnDemoEN" component={Demo} durationInFrames={durationInFrames} fps={fps} width={1920} height={1080} defaultProps={{ language: "en" }} />
      <Composition id="ConnDemoKO" component={Demo} durationInFrames={durationInFrames} fps={fps} width={1920} height={1080} defaultProps={{ language: "ko" }} />
      <Still id="ConnPoster" component={Poster} width={1920} height={1080} />
    </>
  );
};

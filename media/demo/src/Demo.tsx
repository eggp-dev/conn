import { TransitionSeries } from "@remotion/transitions";
import type { Language } from "./copy";
import { clips, closingDurationInFrames } from "./footage";
import { Intro } from "./scenes/Intro";
import { Request } from "./scenes/Request";
import { Fix } from "./scenes/Fix";
import { Reclaim } from "./scenes/Reclaim";
import { Resume } from "./scenes/Resume";
import { Timeline } from "./scenes/Timeline";
import { Closing } from "./scenes/Closing";

// Clean cuts preserve the fixed geometry and avoid dissolving terminal text.
export const Demo: React.FC<{ language: Language }> = ({ language }) => <TransitionSeries>
  <TransitionSeries.Sequence durationInFrames={clips.intro.durationInFrames} name="Ask your agent"><Intro language={language} /></TransitionSeries.Sequence>
  <TransitionSeries.Sequence durationInFrames={clips.request.durationInFrames} name="Read shared context"><Request language={language} /></TransitionSeries.Sequence>
  <TransitionSeries.Sequence durationInFrames={clips.fix.durationInFrames} name="Agent works"><Fix language={language} /></TransitionSeries.Sequence>
  <TransitionSeries.Sequence durationInFrames={clips.reclaim.durationInFrames} name="Human adds a case"><Reclaim language={language} /></TransitionSeries.Sequence>
  <TransitionSeries.Sequence durationInFrames={clips.resume.durationInFrames} name="Agent reads the change and continues"><Resume language={language} /></TransitionSeries.Sequence>
  <TransitionSeries.Sequence durationInFrames={clips.timeline.durationInFrames} name="Review the collaboration"><Timeline language={language} /></TransitionSeries.Sequence>
  <TransitionSeries.Sequence durationInFrames={closingDurationInFrames} name="Conn"><Closing language={language} /></TransitionSeries.Sequence>
</TransitionSeries>;

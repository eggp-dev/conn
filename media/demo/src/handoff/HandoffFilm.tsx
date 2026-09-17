import { TransitionSeries } from "@remotion/transitions";
import { ClosingScene } from "./ClosingScene";
import { RecordedScene } from "./RecordedScene";
import { handoffClips, handoffCopy, hookTrim, shortTrim, type HandoffLanguage } from "./edit";

/** Clean cuts; the 30-second handoff remains one source sequence. */
export const HandoffFilm: React.FC<{ language: HandoffLanguage }> = ({ language }) => {
  const cues = handoffCopy[language].main;
  return <TransitionSeries>
    <TransitionSeries.Sequence durationInFrames={180} name="The correction">
      <RecordedScene file={handoffClips.handoff.file} trim={hookTrim} cues={cues} />
    </TransitionSeries.Sequence>
    <TransitionSeries.Sequence durationInFrames={570} name="A moment earlier: ask your agent">
      <RecordedScene file={handoffClips.request.file} trim={handoffClips.request.trim} cues={cues} offset={6} />
    </TransitionSeries.Sequence>
    <TransitionSeries.Sequence durationInFrames={900} name="Human correction, fresh observation, continuation">
      <RecordedScene file={handoffClips.handoff.file} trim={handoffClips.handoff.trim} cues={cues} offset={25} />
    </TransitionSeries.Sequence>
    <TransitionSeries.Sequence durationInFrames={270} name="Check the actual result">
      <RecordedScene file={handoffClips.result.file} trim={handoffClips.result.trim} cues={cues} offset={55} />
    </TransitionSeries.Sequence>
    <TransitionSeries.Sequence durationInFrames={180} name="Keep your agent. Share your terminal.">
      <ClosingScene language={language} />
    </TransitionSeries.Sequence>
  </TransitionSeries>;
};

export const HandoffShort: React.FC<{ language: HandoffLanguage }> = ({ language }) => <TransitionSeries>
  <TransitionSeries.Sequence durationInFrames={180} name="Human corrects the directory">
    <RecordedScene file={handoffClips.handoff.file} trim={shortTrim} cues={handoffCopy[language].short} />
  </TransitionSeries.Sequence>
  <TransitionSeries.Sequence durationInFrames={180} name="Agent continues and confirms the result">
    <RecordedScene file={handoffClips.result.file} trim={0} cues={handoffCopy[language].short} offset={6} />
  </TransitionSeries.Sequence>
  <TransitionSeries.Sequence durationInFrames={90} name="Try Conn">
    <ClosingScene language={language} short />
  </TransitionSeries.Sequence>
</TransitionSeries>;

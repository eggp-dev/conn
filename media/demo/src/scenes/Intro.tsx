import { FootageStage } from "../FootageStage";
import type { Language } from "../copy";
export const Intro: React.FC<{ language: Language }> = ({ language }) => <FootageStage id="intro" language={language} accent="#b5aaff" />;

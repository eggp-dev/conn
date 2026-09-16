import { FootageStage } from "../FootageStage";
import type { Language } from "../copy";
export const Timeline: React.FC<{ language: Language }> = ({ language }) => <FootageStage id="timeline" language={language} accent="#b5aaff" />;

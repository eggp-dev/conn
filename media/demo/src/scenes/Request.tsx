import { FootageStage } from "../FootageStage";
import type { Language } from "../copy";
export const Request: React.FC<{ language: Language }> = ({ language }) => <FootageStage id="request" language={language} accent="#e9ad6b" />;

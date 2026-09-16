import { AbsoluteFill } from "remotion";
import { fontFamily } from "./fonts";

export const Stage: React.FC<React.PropsWithChildren> = ({ children }) => <AbsoluteFill style={{
  background: "#080a10", color: "#f2f2f7", fontFamily,
}}>
  {children}
</AbsoluteFill>;

import { loadFont } from "@remotion/fonts";
import { staticFile } from "remotion";

export const fontFamily = "Conn Noto, sans-serif";
loadFont({ family: "Conn Noto", url: staticFile("fonts/ConnNoto-Regular.woff2"), weight: "400" });
loadFont({ family: "Conn Noto", url: staticFile("fonts/ConnNoto-Bold.woff2"), weight: "700" });

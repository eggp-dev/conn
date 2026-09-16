import { Easing, interpolate, useCurrentFrame } from "remotion";

/** Faithful vector path from the product's Conn icon, with frame-driven light. */
export const BrandMark: React.FC<{ size?: number; color?: string; animated?: boolean }> = ({
  size = 44, color = "#b5aaff", animated = true,
}) => {
  const frame = useCurrentFrame();
  return <svg width={size} height={size} viewBox="0 0 64 64" aria-label="Conn">
    <path d="M44 16 A21 21 0 1 0 44 48" fill="none" stroke={color} strokeWidth="8" strokeLinecap="round"
      style={{ opacity: animated ? interpolate(frame % 160, [0, 80, 160], [0.78, 1, 0.78], { easing: Easing.bezier(0.4, 0, 0.6, 1) }) : 1 }} />
    <rect x="42" y="26" width="8" height="12" rx="4" fill="#eeeaff" />
  </svg>;
};

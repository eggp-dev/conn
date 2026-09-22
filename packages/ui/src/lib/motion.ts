import { fly } from "svelte/transition";
import { cubicIn, cubicOut } from "svelte/easing";

/** Conn dock motion: reserve layout once, animate only the rendered surface. */
export const DOCK_MOTION_MS = 320;
export const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;
export function decisionReveal(node: HTMLElement) {
  return fly(node, { y: -10, duration: reducedMotion() ? 0 : 180 });
}
export function dockReveal(node: HTMLElement) {
  return fly(node, { duration: reducedMotion() ? 0 : DOCK_MOTION_MS, easing: cubicOut, y: node.getBoundingClientRect().height + 10, opacity: 1 });
}

// Svelte runs outro progress backwards; mirror easing to keep the actual
// movement cubic-out, matching the terminal on both opening and closing.
export function dockDismiss(node: HTMLElement) {
  return fly(node, { duration: reducedMotion() ? 0 : DOCK_MOTION_MS, easing: cubicIn, y: node.getBoundingClientRect().height + 10, opacity: 1 });
}

export function dockOffsetFrames(distance: number): Keyframe[] {
  return Array.from({ length: 33 }, (_, index) => {
    const progress = index / 32;
    return { offset: progress, transform: `translateY(${distance * (1 - cubicOut(progress))}px)` };
  });
}

/** Only the surface morphs; text fades independently and the PTY never resizes. */
export function badgeMorph(node: HTMLElement, anchor: HTMLElement | undefined) {
  const to = node.getBoundingClientRect();
  const from = anchor?.getBoundingClientRect() ?? to;
  return { duration: reducedMotion() ? 0 : 320, easing: cubicOut,
    css: (t: number) => `--morph-x:${(from.left-to.left)*(1-t)}px;--morph-y:${(from.top-to.top)*(1-t)}px;--morph-sx:${t+(1-t)*from.width/Math.max(1,to.width)};--morph-sy:${t+(1-t)*from.height/Math.max(1,to.height)};--morph-radius:${14+30*(1-t)}px;--morph-content:${Math.max(0,(t-.55)/.45)};`
  };
}

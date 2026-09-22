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

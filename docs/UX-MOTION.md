# Conn dock motion / 공간을 여는 보조 UI

## Principle / 원칙

Supporting UI makes room for the terminal instead of covering its working area.
보조 UI는 터미널 작업 영역 위를 덮는 대신 필요한 공간을 열고, 닫힐 때 돌려준다.

Motion serves the [human-manageable collaboration invariant](PRD.md#human-manageable-collaboration--product-invariant).
움직임은 [사람이 관리할 수 있는 협업 불변식](PRD.ko.md)을 따른다.
Preserve the existing visual language; make a new decision distinguishable by its
meaning, scope and action, not by giving every request a different animation.
기존 시각 언어는 유지한다. 새 판단은 의미·범위·행동으로 구별하며 요청마다 다른 애니메이션을 만들지 않는다.
The [control experience proposal](control-experience.md) defines notification,
decision and persistent-status roles; those proposed changes are not implemented yet.
[제어 경험 제안](control-experience.ko.md)은 알림·판단·상시 상태의 역할을 정하며 아직 구현되지 않았다.

## Control-request direction / 제어권 요청 방향 (proposed / 구현 전)

The existing top-right Conn badge expands into the control request and returns to
its source when collapsed, following the continuity of
[Morphing Dialog](https://motion-primitives.com/docs/morphing-dialog).
오른쪽 Conn 뱃지가 제어권 요청으로 확장되고 접으면 원래 자리로 돌아온다.
위치·크기·모서리의 연속성을 참고하며 액체 모양이나 연결 줄기를 그리지 않는다.
Command review keeps the bottom dock. Consistency means predictable motion for
the same decision, while different decisions remain distinguishable.
명령 실행 승인은 기존 하단 도크를 유지한다. 같은 판단은 같은 움직임으로, 다른 판단은 구별되게 표현한다.
Dismissal is not denial; unresolved requests remain in the badge. The prototype
must settle modality, placement, keyboard focus and reduced motion without using
animation geometry as a stream of PTY resize events.
접기는 거절이 아니며 미해결 요청은 뱃지에 남는다. 모달 여부·위치·키보드 포커스·동작 줄이기는
프로토타입에서 검증하고, 시각적 크기 변화를 연속 PTY 리사이즈로 전달하지 않는다.

## Shared behavior / 공통 동작

- The handback hint and timeline use the same bottom-dock motion.
  제어권 반환 안내와 타임라인은 동일한 하단 도킹 동작을 사용한다.
- Opening reserves the final space; closing restores it. The terminal grid is fitted to the final geometry, while its visual offset animates.
  최종 공간을 먼저 계산하고 터미널의 시각적 위치만 움직인다.
- Move the panel from below by its full height; keep it opaque so it pushes into place rather than floating over the terminal.
  패널은 자기 높이만큼 아래에서 올라오며 페이드 없이 셸을 밀어 올리는 경계를 유지한다.
- Animate compositor transforms only: 320 ms, cubic-out, without bounce or overshoot.
  320ms 감속 전환을 사용하며 튀거나 목표 위치를 넘기는 움직임은 넣지 않는다.
- Never animate layout height frame by frame: that repeatedly resizes the PTY and redraws shell applications.
  높이를 프레임마다 변경해 PTY 재계산과 셸 재출력을 유발하지 않는다.
- Escape dismisses supporting UI while it is visible. Otherwise terminal Escape behavior is preserved.
  보조 UI가 보일 때 Esc로 닫고, 평소에는 셸의 Esc 동작을 유지한다.
- Respect reduced motion. No permanent GPU layer hints; will-change is used only during terminal motion.
  동작 줄이기 설정을 존중하며 합성 레이어 힌트를 상시 유지하지 않는다.
- Timeline has a stable, viewport-bounded height and internal scrolling. New records or expanded details must not keep pushing the shell around.
  타임라인은 화면 크기에 맞춘 일정한 높이와 내부 스크롤을 사용한다. 기록 추가나 상세 펼침으로 셸을 계속 밀지 않는다.
- When both are visible, the timeline sits above the handback hint and both reserve space.
  반환 안내와 타임라인이 함께 열리면 아래에서부터 쌓고 두 영역 모두 공간을 확보한다.

## Implementation / 구현

Shared tokens and surface transition: `packages/ui/src/lib/motion.ts`.
Dock geometry: `App.svelte`. Terminal visual offset: `components/Term.svelte`.
This is a product motion convention, not a claim that hardware acceleration is enabled on every device.

## Review checklist / 확인 기준

- Open/close with the prompt at the bottom; the settled layout never covers it.
- Repeated toggle and Esc leave the correct terminal size and no lingering transform.
- New entries and expanded details scroll inside the timeline.
- Check short windows, two docks together, tab switches and reduced motion.
- Profile actual frame time and PTY resize counts before making performance claims.

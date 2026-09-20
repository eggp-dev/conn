# Session screen hard cut / 공유 셸 화면 전환

## Contract

- Observation belongs to an explicitly shared shell session, independent of focus,
  minimization, other tabs, settings panels or renderer heartbeat.
- Core parses PTY output into the current terminal grid. Snapshots use
  that single projection. Raw input, scrollback and process memory are excluded.
- Human input preempts control. Sharing revocation, disconnect, process exit, leases,
  policy and approvals retain their existing enforcement.
- Frontend frame publication and focus-based execution suspension are removed.
- Text observation is not a screenshot: theme-dependent visual effects and graphics
  are not confidentiality controls. Use no-echo input for secrets.

## 한국어

관찰 기준은 포커스된 창이 아니라 명시적으로 공유한 셸 세션입니다. 다른 탭이나
앱으로 이동해도 같은 셸의 현재 화면을 읽고, 승인된 제어권으로 작업할 수 있습니다.
사람이 직접 입력하면 즉시 제어권을 회수합니다. 공유 해제·연결 종료·프로세스 종료와
기존 승인 및 정책 검사는 유지합니다.

화면은 코어에서 셸 출력을 해석해 구성합니다. 원시 입력, 스크롤백, 환경변수나
프로세스 메모리는 전달하지 않습니다. 무표시 암호는 나타나지 않고, 별표 암호는
별표로만 나타납니다. 프로그램이 평문으로 출력한 비밀은 공유 대상입니다.
이 계약은 창의 픽셀 스크린샷이 아니므로 색상이나 테마로 비밀을 숨기면 안 됩니다.

## Platform verification

Linux core, socket, real PTY and browser tests must be rerun for this change.
Earlier macOS foreground-denial results are historical, not acceptance evidence.
macOS and Windows native focus/minimize checks remain required before release.

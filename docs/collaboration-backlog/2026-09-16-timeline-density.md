# COLLAB-003 · 하단 타임라인 밀도 / Timeline strip density

## 목적과 환경

- 날짜: 2026-09-16 (KST).
- 장시간 협업 중 하단 타임라인의 가독성과 항목 선택 경험에 대한 사용자 피드백.
- 근거: 사용자가 제공한 화면. `Timeline 75` 표시와 촘촘하게 나열된 막대·고리 형태의 항목이 보인다.
- 정확한 앱 버전, 전체 창 크기, CSS 픽셀 치수는 이 관찰에서 측정하지 않았다.

## 실제 진행과 관찰

| ID | 행동·사람의 개입 | 관찰 결과 | 근거 |
|---|---|---|---|
| O1 | 협업 기록이 쌓인 뒤 하단 타임라인 확인 | 항목이 좁은 폭으로 밀집해 개별 기록을 구분하고 포인터로 선택하기 어려워 보임 | 사용자 스크린샷 및 피드백; 클릭 실패율은 미측정 |

## 사용자 피드백

직접 인용: “아래 타임라인이 너무 촘촘하게 들어가는데 최소 픽셀은 좀 보장해주는게 좋을거같아.”

## 개선 요구사항 · UX-C003

- 기록 수가 늘어나도 각 항목의 **최소 표시 너비와 항목 간 간격**을 보장한다.
- 작은 표시와 별개로 **최소 클릭·호버 영역**을 확보한다. 인접 항목의 선택 영역이 겹쳐 엉뚱한 기록이 열리지 않아야 한다.
- 공간이 부족할 때 항목을 계속 압축하지 않는다. 가로 스크롤이나 구간 탐색 등 넘침 처리 방식을 검토한다.
- 현재 타임라인의 명령·협업 기록 구분과 상세 보기 연결을 유지한다.
- 최소 CSS 픽셀 값과 넘침 처리 방식은 실제 크기의 시안 및 조작 검증 후 결정한다. 아직 확정된 수치는 없다.

## 수용 기준과 다음 검증

- 10 / 75 / 200개 기록, 좁은 창과 넓은 창에서 정한 최소 너비·간격이 유지된다.
- 인접한 명령·협업 이벤트를 각각 정확히 호버·클릭하고 해당 상세 내용을 열 수 있다.
- 최신 기록뿐 아니라 오래된 기록도 탐색할 수 있다.
- 새 기록이 추가될 때 과거 기록을 보고 있던 사용자의 위치가 갑자기 이동하지 않는다.

## 상태와 한계

- **관찰됨 / 백로그 등록. 구현은 아직 하지 않음.**
- 본문은 하단 스트립에 관한 피드백이며, 펼친 타임라인 패널의 카드 높이 문제와 구분한다.
- 원본 화면은 터미널 내용이 포함되어 저장소에 복사하지 않았다.
- 재검증: 아직 없음.

## English summary

The bottom timeline strip becomes too dense during long collaboration sessions (75 records in the supplied screenshot). Preserve a minimum visible width, spacing, and usable pointer target for each item. Evaluate overflow navigation instead of indefinitely shrinking items. Validate with 10, 75, and 200 records at different window widths. Pixel values and the overflow approach remain undecided. Backlog only; not implemented.

## 2026-09-17 implementation / 구현 후 검증

- v0.6.0 candidate: fixed 24 × 24 CSS-pixel targets, 4px gaps, 16px command marks and 8px collaboration rings. Horizontal scrolling, previous/recent navigation and arrow-key navigation preserve individual targets.
- Browser measurements: 10, 75 and 200 records all retain 24px targets. A 720 × 520 viewport has no document overflow. After trimming the oldest record, the same inspected record retained its position within 0.2px when a new command arrived.
- A new shell starts with zero timeline records. Signed native UI and device-specific frame timing remain separate checks.
- 구현 및 브라우저 확인을 마쳤으며 네이티브 릴리스 검증은 별도다. 위 본문은 최초 관찰 기록으로 보존한다.

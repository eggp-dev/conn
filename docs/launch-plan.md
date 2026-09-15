# First public launch / 첫 공개 계획

## The story / 전달할 가치

**One terminal. Two collaborators. You keep the conn.** Show a human and an agent sharing the same working session: request → inspect → approve → interrupt → review the timeline. A new visitor should understand this before reading the architecture.

**터미널 하나, 협업자 둘, 제어권은 내 손에.** 요청·검토·승인·중단·타임라인을 한 흐름으로 보여줍니다. 구조 설명보다 실제 협업 장면이 먼저입니다.

## A 45-second demo / 45초 데모

Record only a disposable project, with no credentials, personal paths or private history. This is a real workflow recording to produce after platform smoke tests, not a simulated benchmark.

| Time | Scene / 장면 |
|---|---|
| 0–8s | Human opens a shell; agent requests control to read a demo file / 에이전트가 데모 파일 읽기 제어권 요청 |
| 8–18s | Inspect the original command, approve, see the result / 원문 확인·승인·결과 |
| 18–28s | Agent asks to delete it; human denies / 삭제 요청을 거절 |
| 28–38s | A new approved command enters grace; human takes back control / 실행 유예 중 제어권 회수 |
| 38–45s | Expand the denied request in Timeline; show both languages briefly / 거절 원문과 영·한 UI |

Use matching English and Korean captions. The icon motion should be visible without fast cuts. Add the verified recording to both README pages when ready; do not label an illustration as a live demo.

## Readiness and distribution / 준비와 알리기

1. Land CI and bilingual onboarding; get fresh-install feedback from a small number of real users.
2. Publish a preview only after native installation smoke tests and clear signing notes.
3. Ask testers whether they connected their agent, completed the demo, and understood denied requests. Fix the largest onboarding obstacle before broad promotion.
4. Prepare English and Korean launch copy below. Post only after maintainer approval, following each community's rules.
5. Track stars alongside meaningful adoption: successful first runs, repeat use reports, actionable issues and contributions. **100 stars is the goal, not a forecast or a quality guarantee.** Do not buy stars or send unsolicited bulk promotion.

CI·영한 안내·신규 설치 피드백을 먼저 확보하고 프리뷰를 공개합니다. 첫 연결과 거절 이유 이해에서 막힌 부분을 수정한 뒤 커뮤니티에 알립니다. 별 수와 함께 실제 첫 실행 성공·재사용·유의미한 이슈·기여를 봅니다. 100 stars는 목표이며 예측이나 품질 보증이 아닙니다.

### English launch copy (draft)

> I built Conn because I wanted to work with an AI agent in the terminal I am already using. The agent requests control, proposes a command, and I can review it or take the keyboard back. Commands and collaboration decisions stay together in a timeline, including denied requests and their original payloads. It is an early Rust/Tauri preview with an MCP adapter and English/Korean UI. I would love feedback on the first-run experience and where control handoff feels confusing.

### 한국어 공개 글 (초안)

> AI 에이전트와 내가 쓰던 터미널에서 직접 협업하고 싶어 Conn을 만들었습니다. 에이전트가 제어권과 명령 실행을 요청하면 내용을 보고 승인하거나 키보드로 다시 제어권을 가져올 수 있습니다. 실행한 명령과 거절한 요청·원문을 하나의 타임라인에서 볼 수 있습니다. Rust/Tauri 기반 초기 프리뷰이고 MCP 어댑터와 영·한 UI를 제공합니다. 첫 연결 과정과 제어권을 주고받는 UX에서 불편한 점을 듣고 싶습니다.

The possible `conn.eggp.dev` site remains a separate future approval. This plan creates no website, posts, outreach or recurring automation.

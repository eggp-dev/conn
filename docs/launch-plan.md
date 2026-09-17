# Conn launch plan

Updated 2026-09-17 · Produced locally; publication and independent first-use review pending

[English launch kit](launch-kit.md) · [한국어 공개 자료](launch-kit.ko.md) · [First collaboration](first-collaboration.md)

## One experience to communicate

**Keep your agent. Share your terminal.**

The agent works. You correct the working directory directly. You ask it to read the changed screen and continue there. This is the lead story across the README, film and first-use example.

It comes from the [September 16 collaboration](collaboration-backlog/2026-09-16-remote-codex.md): the user changed the destination before execution, and the agent resumed from the new location. The public demonstration recreates that interaction in a disposable local workspace. It does not depend on waiting for a model to make a spontaneous mistake.

Primary audience: developers already using Codex or Claude Code who want to stay involved in terminal work. Cursor and GitHub Copilot receive the same connection path. Conn adds the shared shell; it does not replace the client's conversation or model account.

## Deliverables and acceptance

| Deliverable | Purpose | Review before publication |
|---|---|---|
| README EN/KO | Explain the workflow and lead to installation | A newcomer can find their installer and describe the handoff |
| Main film EN/KO | Show the correction and real continuation | New state is read before the resulting write; result is visible |
| Short film EN/KO | Make the same story legible in about 15 seconds | Silent captions, stable framing, no invented continuation |
| First collaboration EN/KO | Let the reader reproduce the value locally | One disposable workspace, one output file, no SSH prerequisite |
| Client connection guide EN/KO | Connect an existing agent | Settings labels and local/cloud boundaries match the app |
| Posters and stills | Explain the two-app relationship at small sizes | Both apps identifiable; no credentials or personal paths |
| FAQ EN/KO | Resolve practical questions without crowding README | Control, screen sharing and command history are distinguished |
| Launch kit EN/KO | Prepare a clear story and feedback request | Claims match the recording; publication is still a separate action |
| Recording provenance | Make the demo's evidence inspectable | Version, environment, real model run and editing method recorded |

The main film uses `docs/assets/conn-handoff-en.mp4` and `conn-handoff-ko.mp4`, with language-specific posters. The existing test-fix film remains a secondary example. Production details belong in [the media project](../media/demo/README.md) and [demo notes](demo.md), not the README opening.

## Production status

Prepared locally on September 17:

- Bilingual README, first-collaboration guide, FAQ, story and launch copy.
- Main videos: 70 seconds each; short videos: 15 seconds each. All four are 1920×1080 at 30fps, silent with localized editorial text.
- Four poster/social covers, three actual capture stills, eight SRT/VTT subtitle files.
- A real Codex/Conn run verified the corrected destination, exact one-line result, absence of an output in the original directory, and returned control.
- Video decode, font coverage, editorial timing, source links and the shell fixture passed local checks.

The rendered assets and their limits are described in [demo notes](demo.md). No public attachment, community post, website, independent tester result or new app release is created by this production work.

## Film direction

Target length: 60–75 seconds, with a 12–15-second companion cut. Adjust timing to readable actual footage; the target is not permission to imply an action happened before it did.

| Beat | What the viewer sees |
|---|---|
| Hook | “I changed the directory. Continue from here.” alongside the correction |
| Context | Actual Codex and Conn, clearly labelled as separate applications |
| Agent turn | Agent reads the shared shell and prepares a small task |
| Intervention | Direct terminal input returns control; user role changes the destination |
| Continuation | Agent re-reads the screen, checks location and writes in the new folder |
| Result | File and location are verified; control returns |
| Close | Product message, repository and first-collaboration invitation |

The completed edit keeps the two-app view stable, with no zooms or pans. Captions use their own gutter, never the command line. Preserve the visible causal order through the intervention and continuation; shorten model waits explicitly.

Current production uses a genuine Codex run with **automated user-role actions**, not an independent human participant. State this in film notes and captions. Record the exact Conn source/version and backend in provenance. Browser-adapter footage using the native backend is evidence for the captured interaction, not a downloaded native installer test.

Do not make authentication, nested remote agents or external automation prerequisites for understanding the story. Those are separate follow-up cases. Public clips use disposable data from the start.

## Publication sequence

1. Review the short cut for message clarity, then the complete film for readability and provenance.
2. Have five people who already use a local agent follow the first-collaboration guide without coaching. Record confusion and assistance, with consent.
3. Fix the largest obstacle. Recheck the download, connection and example links in both languages.
4. Publish a Korean account of the collaboration, with a runnable example and one concrete feedback question.
5. Incorporate feedback, then adapt the English draft to the chosen community. Be available to answer questions.
6. Follow with a real user case or a solved onboarding problem; do not repeatedly post identical promotion.

No outside testers have been recruited or results measured by writing this plan. Community posts, hosted video attachments and `conn.eggp.dev` are separate publication actions. Read each destination's current rules before posting. For Show HN, start with its [official guidelines](https://news.ycombinator.com/showhn.html); do not coordinate votes or comments.

## Measure usefulness alongside stars

**100+ GitHub stars is the discovery goal, not a forecast.** The following are proposed small-sample gates, not measured outcomes or statistical guarantees:

- Four of five newcomers can explain the shared-shell correction after 15 seconds.
- Four of five connect their agent and finish the example using the guide.
- Three of five voluntarily report using Conn again for real work within a week.

Track where each attempt stops: understanding, installation, connection, takeover, or continuation. Record elapsed time and help needed. Use aggregate [GitHub traffic](https://docs.github.com/en/repositories/viewing-activity-and-data-for-your-repository/viewing-traffic-to-a-repository) and voluntary reports; downloads include maintainer/CI activity and are not unique users. Do not add terminal, credential or command collection for marketing measurement.

## 한국어 요약

**쓰던 에이전트와, 같은 터미널에서.** 에이전트의 작업 → 사람의 경로 보정 → 바뀐 화면을 읽고 재개하는 경험을 README·영상·첫 체험에 일관되게 담습니다.

- 다운로드와 첫 체험이 우선이며, 모드·정책·자동화의 상세 설명은 별도 문서로 보냅니다.
- 실제 Codex와 Conn을 촬영합니다. 사람 역할을 자동화한 재현임을 밝히고, 대기 시간 편집과 촬영 환경을 기록합니다.
- 영상·가이드를 먼저 완성하고 5명 체험 후 공개 글을 다듬습니다. 실제 테스터 결과는 아직 없습니다.
- 목표는 100+ stars입니다. 첫 연결 성공·직접 보정·재사용을 함께 확인하며 달성을 보장하지 않습니다.
- 커뮤니티 게시·새 사이트 배포는 이 문서 작성이나 로컬 제작에 포함되지 않습니다.

# 주인공 장면 재기획: "내가 접속한 서버에서, 에이전트가 이어받고, 내가 막는다"

상태: 기획안 · 2026-09-19 · 아직 촬영하지 않음. 기존 제작 방식은 [README](README.md)와 [HANDOFF](HANDOFF.md)를 따릅니다.

## 왜 장면을 바꾸나

지금 주인공 영상은 "에이전트가 엉뚱한 폴더에서 시작해서 내가 `cd`로 고쳤다"입니다. 제품 동작은 정확히 보여 주지만, 처음 보는 사람에게는 앱을 하나 더 설치할 이유가 되지 못합니다. 그 정도는 에이전트에게 말로 해도 되기 때문입니다.

새 장면은 다른 도구로는 보여 주기 어려운 세 가지를 30초 안에 연달아 보여 줍니다.

1. **에이전트가 스스로 갈 수 없는 곳.** 내가 직접 로그인한 SSH 세션을 에이전트가 이어받습니다. 암호는 에이전트에게 가지 않습니다.
2. **위험한 명령은 나를 기다린다.** `sudo`와 `rm -rf`에서 승인 카드가 뜹니다.
3. **입력하면 바로 내 것.** 에이전트가 틀린 방향으로 가는 순간 키보드를 치면 제어권이 돌아옵니다.

한 문장으로: **"Your agent works in your terminal. You keep the keyboard."** README 첫 화면의 네 항목이 이 장면의 자막과 일대일로 맞습니다.

## 장면 구성 (본편 약 35초)

화면은 기존 영상과 같습니다. 왼쪽은 실제 에이전트 클라이언트, 오른쪽은 Conn입니다.

| 시간 | 화면 | 자막 (EN / KO) |
|---|---|---|
| 0–3초 | Conn에서 사람이 `ssh deploy@staging`. 암호 프롬프트에 입력(화면에 글자 없음). 원격 프롬프트가 뜸 | You log in. The password never reaches the agent. / 로그인은 내가 합니다. 암호는 에이전트에게 가지 않습니다. |
| 3–8초 | 에이전트 클라이언트에 입력: "staging의 API가 502를 돌려줘. 내 터미널을 보고 원인을 찾아 줘." 에이전트가 스냅샷을 읽고 이유를 적어 제어권 요청. Conn 상단에 이유가 표시되고 에이전트 색으로 바뀜 | Your agent picks up the shell you are in. / 에이전트가 내 셸을 이어받습니다. |
| 8–15초 | 에이전트가 원격에서 `./api.sh status`, `tail -n 20 api.log` 실행. 타임라인에 명령마다 한 줄 이유가 붙음 | Every command comes with a reason. / 모든 명령에 이유가 붙습니다. |
| 15–21초 | 에이전트가 `sudo ./api.sh restart` 입력 → 승인 카드 "privilege escalation"과 이유. 사람이 **Approve**. 서비스 재시작 | Risky commands wait for you. / 위험한 명령은 나를 기다립니다. |
| 21–28초 | 에이전트가 "디스크를 비우겠다"며 `rm -rf logs/` 입력 → 승인 카드 "recursive delete". 사람이 **Deny**를 누르고 곧바로 `df -h .`를 직접 입력. 상단 표시가 "you have the conn"으로 바뀜. 디스크는 넉넉함 | Type to take over. / 입력하면 바로 넘겨받습니다. |
| 28–35초 | 에이전트 클라이언트에 입력: "로그는 지우지 마. 터미널을 읽고 이어서 해." 에이전트가 화면을 다시 읽고 `curl -s localhost:8080/health` → `200 OK`. 제어권 반납 | It reads what you changed and continues. / 바뀐 화면을 읽고 이어갑니다. |
| 끝 | 로고와 한 줄 | Your agent works in your terminal. You keep the keyboard. |

**15초 편집본**은 0–3초, 15–21초, 21–28초 세 구간만 씁니다(로그인, 승인, 개입). README 맨 위의 자동 재생 미리보기는 이 15초본에서 만듭니다.

## 촬영 환경

- **원격 서버는 일회용 컨테이너**로 만듭니다. OpenSSH 서버, `deploy` 계정, 합성 암호, 가짜 서비스 스크립트 `api.sh`(`status`/`restart`, 작은 HTTP 서버를 띄우고 내림), 미리 넣어 둔 `api.log`(502 원인이 보이는 몇 줄), `./api.sh restart`에만 허용된 sudo. 실제 서버, 실제 자격증명, 실제 호스트 이름은 쓰지 않습니다.
- 기존 원칙을 그대로 지킵니다. **실제 에이전트 세션과 실제 Conn**을 쓰고, 사람 역할 입력은 자동화할 수 있으며, 대기 시간은 편집합니다. 영상 아래 설명에 이 사실을 적습니다.
- 에이전트가 `rm -rf`를 시도하는 대목은 연출입니다. 에이전트에게 주는 과제 문구에 "디스크 여유가 부족해 보이면 로그를 정리해도 된다"를 넣어 자연스럽게 유도하고, 설명에 "준비된 재현"임을 밝힙니다.
- 정책은 기본값 그대로 씁니다(`sudo` = privilege escalation, `rm -r` = recursive delete). 영상용으로 정책을 바꾸지 않습니다.

## 촬영 전에 확인할 것

- [ ] **승인 대기 중 사람 입력.** 2026-09-19에 확인한 결함: 승인 카드가 떠 있는 동안 사람이 키를 누르면 승인이 취소되지 않고, 입력 줄에 사람이 친 글자가 섞인 채로 남습니다. 그 상태에서 승인하면 카드에 보인 것과 다른 명령이 실행됩니다. 21–28초 구간이 바로 이 상황이므로 **수정이 먼저**입니다(사람 입력이 대기 중인 승인을 거부 처리하고 입력 줄을 비우도록).
- [ ] Deny 직후 입력 줄이 비워지는지(현재 Ctrl-U를 보냄), 원격 셸에서도 같은지.
- [ ] SSH 세션 안에서 승인 카드의 라벨과 이유가 로컬과 같게 뜨는지.
- [ ] 새 수락 카드("wants to join")가 촬영 흐름을 끊지 않도록, 촬영 시작 전에 허용해 두거나 첫 1초에 포함할지 결정.
- [ ] 자막의 주장마다 근거 테스트가 있는지: 숨김 입력 비노출(`input_privacy.rs`, `real_ssh_login…`), 사람 선점(`control_regressions.rs`), 승인(`guard.rs`, `session.rs`).

## 산출물과 README 교체

- `docs/assets/conn-remote-{en,ko}.mp4`(본편), `conn-remote-short-{en,ko}.mp4`(15초), 자막 `.vtt`/`.srt`, 포스터와 소셜 이미지.
- 미리보기: 15초본에서 960px·8fps 애니메이션 WebP(언어별 약 2MB)를 만들어 `conn-remote-preview-{en,ko}.webp`로 저장하고 README 두 곳의 파일 이름과 설명 문구만 바꿉니다. 만드는 명령은 `conn-handoff-preview-*`와 같습니다.
- 본편 mp4를 GitHub 댓글 입력창에 끌어다 놓아 얻은 주소를 README에 한 줄로 두면 재생기가 붙습니다.
- 기존 폴더 교정 영상은 지우지 않고 "다른 협업" 자리로 내립니다.

## 같은 장면을 쓰는 곳

- Show HN / Reddit 글의 첫 문단과 첨부 영상.
- 저장소 About 문구: "Your agent works in your terminal. You keep the keyboard. A shared terminal with approvals, takeover and an audit trail for Codex, Claude Code, Cursor and Copilot."
- 소셜 미리보기 이미지: 승인 카드가 떠 있는 21초 지점의 정지 화면.

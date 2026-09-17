# 첫 협업 해보기

[English](first-collaboration.md) · 한국어 · [Conn으로 돌아가기](../README.ko.md)

**작업 위치를 직접 바꾸고, 에이전트에게 거기서 이어가도록 해보세요.**

임시 폴더 두 개와 텍스트 파일 하나로 체험합니다. 대화는 쓰던 에이전트에서, 함께 쓰는 터미널은 Conn에서 진행합니다.

## 시작 전에

Conn을 열고 [에이전트를 연결](agent-integrations.ko.md)하세요. Conn 오른쪽 위 제어 메뉴에서 **Autopilot**을 선택하고 **넘길 때 묻기**를 켜세요. 영어 UI에서는 **Ask before granting**입니다. 체험 중에는 이 탭을 화면에 유지합니다.

macOS/Linux에서는 로컬 Bash 또는 Zsh 프로필을, Windows에서는 PowerShell 프로필을 사용하세요. 아래에서 셸에 맞는 명령을 선택하면 됩니다.

## 1. 빈 폴더 두 개 준비하기

아래 명령을 **Conn 터미널에** 붙여 넣으세요. 실행할 때마다 새로운 임시 폴더를 만들므로, 파일을 지우지 않고 다시 체험할 수 있습니다.

<details open>
<summary>macOS / Linux · Bash 또는 Zsh</summary>

```sh
conn_demo_dir=$(mktemp -d "${TMPDIR:-/tmp}/conn-demo.XXXXXX") &&
mkdir "$conn_demo_dir/draft" "$conn_demo_dir/workspace" &&
cd "$conn_demo_dir/draft" &&
pwd
```

</details>

<details>
<summary>Windows · PowerShell</summary>

```powershell
$connDemoDir = Join-Path ([System.IO.Path]::GetTempPath()) ('conn-demo-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $connDemoDir -ErrorAction Stop | Out-Null
New-Item -ItemType Directory -Path (Join-Path $connDemoDir 'draft'), (Join-Path $connDemoDir 'workspace') -ErrorAction Stop | Out-Null
Set-Location -LiteralPath (Join-Path $connDemoDir 'draft')
Get-Location
```

</details>

출력된 경로가 `draft`로 끝나면 준비됐습니다.

## 2. 에이전트에게 준비시키기

**에이전트 대화창**에 요청하세요.

> 이번 작업은 Conn으로 해줘. 현재 화면을 읽고 정확한 명령을 포함해 제어를 요청한 다음, `pwd`로 현재 폴더를 확인해. 제어권을 유지한 채 내 다음 말을 기다려. 아직 파일을 만들거나 수정하지 마.

Conn에 뜬 요청을 확인하고 허용하세요. 별도의 명령 승인도 나타나면 확인합니다. `draft` 경로가 출력되고 셸 입력줄이 비어 있는 상태까지 기다리세요.

## 3. 직접 위치 바로잡기

**Conn 터미널**에 직접 입력하세요.

```sh
cd ../workspace
pwd
```

위 명령은 앞서 안내한 두 셸 유형 모두에서 사용할 수 있습니다. 직접 입력하면 제어권이 돌아옵니다. 출력된 경로는 이제 `workspace`로 끝납니다.

입력하기 전에 제어권이 만료돼도 체험은 이어갈 수 있습니다. 제어가 넘어오는 순간도 보고 싶다면, 직접 입력할 준비가 됐을 때 읽기 전용 요청을 다시 해보세요.

## 4. 바뀐 화면에서 이어가기

**에이전트 대화창**에 요청하세요.

> Conn에서 내가 경로를 바꿨어. 현재 화면을 다시 읽고 현재 폴더를 확인해. 다른 폴더로 이동하지 말고 여기에 `collaboration.txt`를 만들어서 `We continued from your correction.` 한 줄을 써줘. 파일이 이미 있으면 덮어쓰지 마. 내용을 다시 읽고 `../draft/collaboration.txt`는 없는지도 확인해. 모든 셸 작업은 Conn으로 하고, 정확한 예정 명령을 포함해 제어를 요청한 뒤 끝나면 돌려줘. 내가 거절하거나 다시 제어권을 가져오면 멈춰.

새 요청과 필요한 명령 승인을 확인하세요. 에이전트는 이전 위치로 돌아가지 않고, 사람이 선택한 폴더에서 이어가야 합니다.

## 성공했다면

- `workspace/collaboration.txt`에 `We continued from your correction.`이 있습니다.
- `draft/collaboration.txt`는 없습니다.
- 에이전트가 바뀐 터미널을 확인한 뒤 이어서 작업하고, 끝나면 제어권을 돌려줍니다.

**타임라인**에서 명령과 제어 전환을 확인할 수 있습니다. 제어권을 가져오면 이후 에이전트 입력이 멈춥니다. 이미 실행 중인 프로세스를 종료하는 기능은 아닙니다.

**대화는 쓰던 에이전트에서, 터미널은 함께.** 실제 작업에서도 프로젝트 위치를 고르거나, 결과를 확인하거나, 실행 전 명령을 보정할 때 같은 방식으로 협업해보세요.

[연결 도움말](agent-integrations.ko.md) · [Conn 설치](getting-started.ko.md) · [예제와 검증 메모](../examples/first-collaboration/README.md)

# Desktop Pet

## 1. 프로젝트 개요

Windows 화면 위에서 사용자가 직접 만든 캐릭터가 살아 움직이는 저메모리 Desktop Pet 프로그램을 만든다.

단순히 GIF를 띄우는 프로그램이 아니라 캐릭터가 상태를 가지고 자율 행동하며, 사용자의 마우스 입력과 타이머 같은 유틸리티 기능에도 반응하는 작은 데스크톱 캐릭터 시스템을 목표로 한다.

핵심 방향은 다음과 같다.

- 가능한 한 적은 메모리와 유휴 CPU 사용
- 화면 위를 자유롭게 이동하는 캐릭터
- 클릭·더블클릭·드래그 등 직접 상호작용
- 사용하지 않을 때 스스로 움직이고 쉬고 잠드는 자율 행동
- 완전 랜덤이 아닌 가중치 기반 상태 전이
- 타이머·포모도로 같은 유틸리티와 캐릭터 행동 연결
- 나중에 캐릭터를 쉽게 추가할 수 있는 데이터 중심 구조

## 2. 확정된 기술 방향

### 기본 스택

- 언어: Rust
- 대상: Windows
- 윈도우 처리: Win32 API
- Pet 창: Layered Window
- 캐릭터 표현: 투명 PNG 스프라이트
- 행동 시스템: Weighted State Machine

### 이 방향을 선택한 이유

메모리 사용량을 최우선으로 두기 때문에 Electron은 제외한다.

Tauri는 Electron보다 가볍지만 Windows에서 WebView2를 사용한다. 이 프로젝트는 복잡한 웹 UI가 필요한 프로그램이 아니므로 WebView를 유지할 이유가 적다.

따라서 캐릭터 렌더링, 입력 처리, 상태 머신, 타이머 정도만 필요한 현재 요구에는 Rust + Win32 네이티브 구성이 가장 목적에 맞는다.

초기 구현 난이도는 높아질 수 있지만 장기적으로는 다음 장점이 있다.

- 불필요한 브라우저 런타임 제거
- 낮은 메모리 오버헤드
- Windows 입력·투명 창·트레이 등 직접 제어
- 동작하지 않을 때 업데이트를 최소화하기 쉬움

## 3. v1 기능 범위

### 3.1 Pet Window

- 투명 배경
- 항상 위에 표시
- 캐릭터만 화면에 보이게 처리
- 투명 픽셀 영역은 가능한 경우 아래 프로그램으로 클릭 통과
- 화면 어디든 이동 가능
- 다중 모니터 대응
- DPI 변화 고려
- 화면 밖으로 완전히 사라지지 않도록 경계 보정

### 3.2 캐릭터 애니메이션

기본 행동 후보:

- idle
- walk
- look
- sit
- stretch
- sleep
- wake
- happy
- dragged
- fall
- land
- special

모든 상태가 v1 첫 구현부터 필요한 것은 아니다. 핵심 루프부터 만들고 단계적으로 추가한다.

캐릭터 방향에 따라 좌우 반전이 가능해야 한다.

### 3.3 사용자 상호작용

#### 클릭

캐릭터가 짧은 반응 애니메이션을 수행한다.

예:

- happy
- surprised
- look_at_user

#### 더블클릭

일반 클릭과 다른 특별 반응을 할 수 있다.

#### 드래그

캐릭터를 길게 누르거나 마우스 버튼을 누른 상태에서 들어 올려 이동할 수 있다.

흐름 예시:

```text
idle
→ dragged
→ drop
→ fall
→ land
→ idle
```

추후 클릭 위치별 상호작용도 확장할 수 있다.

예:

- 머리 클릭 → 쓰다듬기
- 몸 클릭 → 일반 반응
- 반복 클릭 → 짜증
- 드래그 → 잡혀 올라감

### 3.4 자율 행동

사용자 입력이 없어도 캐릭터가 일정 시간마다 자신의 다음 행동을 결정한다.

완전 랜덤 대신 현재 상태에 따라 다음 상태 후보와 가중치를 다르게 둔다.

예:

```text
IDLE
 ├─ 45% → idle
 ├─ 25% → walk
 ├─ 15% → look
 ├─ 10% → sit
 └─  5% → special

SIT
 ├─ 40% → idle
 ├─ 30% → stretch
 └─ 30% → sleep

SLEEP
 ├─ 80% → sleep
 └─ 20% → wake
```

실제 값은 플레이 테스트 후 조정한다.

### 3.5 동적 가중치

가중치는 고정값만 사용하지 않는다.

예:

사용자와 오랫동안 상호작용이 없으면:

```text
sleep 가중치 증가
walk 가중치 감소
```

방금 사용자가 클릭했다면:

```text
happy 가중치 증가
sleep 가중치 감소
```

이를 통해 단순 랜덤보다 살아 있는 느낌을 만든다.

### 3.6 이동

캐릭터는 화면 안에서 목적지를 골라 걸어갈 수 있다.

필요 기능:

- 랜덤 목적지 선택
- 현재 위치 → 목적지 이동
- 좌우 방향 변경
- 화면 경계 감지
- 모니터 경계 처리
- 이동 중 다른 상태로 자연스럽게 전환

추후 기능 후보:

- 마우스 따라가기
- 마우스에서 도망가기
- 화면 모서리에 앉기
- 특정 위치 선호

### 3.7 타이머

기본 Countdown Timer를 제공한다.

초기 프리셋 후보:

- 5분
- 10분
- 25분
- 50분
- 직접 입력

타이머는 캐릭터 행동과 연결한다.

예:

```text
25분 집중 시작
→ study 상태

집중 중
→ 책 보기 / 자세 바꾸기 같은 study idle

종료
→ 알림 행동
→ 말풍선 또는 Windows 알림
```

### 3.8 Pomodoro

Countdown 기능이 안정된 뒤 확장한다.

예:

```text
집중 25분
→ study

휴식 5분
→ play / stretch

다음 집중
→ study
```

### 3.9 트레이 메뉴

기본 후보:

```text
Desktop Pet
├─ 타이머
│  ├─ 5분
│  ├─ 10분
│  ├─ 25분
│  ├─ 50분
│  └─ 직접 입력
├─ 캐릭터
├─ 일시정지
├─ 숨기기
├─ 설정
└─ 종료
```

## 4. 향후 유틸리티 후보

v1 핵심 기능이 안정된 뒤 검토한다.

- Reminder
- 반복 타이머
- Pomodoro 세부 설정
- Break Reminder
- 간단한 Todo
- 현재 시간 표시
- 캐릭터별 전용 상호작용
- 시작프로그램 등록
- 여러 캐릭터 지원
- 캐릭터 팩 import

기능을 추가할 때는 Desktop Pet의 핵심 경험과 메모리 사용량을 해치지 않는지 먼저 본다.

## 5. 캐릭터 데이터 구조

캐릭터 리소스는 엔진 코드와 분리한다.

현재 기본 구조:

```text
assets/
└─ characters/
   ├─ _template/
   │  ├─ character.toml
   │  ├─ preview.png
   │  ├─ idle/
   │  ├─ walk/
   │  ├─ dragged/
   │  ├─ happy/
   │  └─ ...
   └─ my_character/
      ├─ character.toml
      ├─ preview.png
      ├─ idle/
      ├─ walk/
      ├─ dragged/
      ├─ happy/
      └─ ...
```

최소 실행 조건은 `idle` PNG 1장이다. 선택 모션이 없으면 `character.toml`의 fallback 또는 기본 `idle` fallback을 사용한다.

현재 로더는 모션 폴더의 PNG 경로만 먼저 수집하고 실제로 표시할 프레임만 디코딩한다. 모든 애니메이션을 시작 시 한꺼번에 메모리에 올리지 않는다.

설정 예:

```toml
name = "My Pet"
author = "user"
version = "0.1.0"

scale = 1.0
anchor_x = 48
anchor_y = 92
default_facing = "right"

[idle]
fps = 4
loop = true

[walk]
fps = 8
loop = true
speed = 90

[fallbacks]
walk = "idle"
dragged = "idle"
happy = "idle"
look = "idle"
```

상세 규칙은 `docs/CHARACTER_PACK.md`, 이미지 생성 프롬프트는 `docs/IMAGE_GEN_TEMPLATE.md`를 기준으로 한다.

## 6. 예상 아키텍처

초기 후보:

```text
src/
├─ main.rs
│
├─ platform/
│  ├─ mod.rs
│  └─ windows.rs
│
├─ window/
│  ├─ mod.rs
│  └─ pet_window.rs
│
├─ asset/
│  ├─ mod.rs
│  └─ character_pack.rs
│
├─ render/
│  ├─ mod.rs
│  ├─ sprite.rs
│  └─ animation.rs
│
├─ pet/
│  ├─ mod.rs
│  ├─ state.rs
│  ├─ behavior.rs
│  └─ movement.rs
│
├─ interaction/
│  ├─ mod.rs
│  ├─ mouse.rs
│  └─ drag.rs
│
├─ utility/
│  ├─ mod.rs
│  ├─ timer.rs
│  └─ pomodoro.rs
│
├─ config/
│  ├─ mod.rs
│  └─ character.rs
│
└─ tray/
   └─ mod.rs
```

초기에는 파일을 필요 이상으로 잘게 쪼개지 않는다. 기능이 실제로 생길 때 모듈을 분리한다.

## 7. 성능 원칙

메모리와 CPU 사용량은 이 프로젝트의 핵심 요구사항이다.

### 렌더링

항상 60 FPS로 루프를 돌리지 않는다.

예:

```text
걷는 중
→ 이동/애니메이션에 필요한 주기로 갱신

idle 애니메이션
→ 해당 스프라이트 FPS에 맞춰 갱신

완전히 정지
→ 렌더링 중지 또는 최소화

상태 변경
→ 즉시 다시 갱신
```

### 이미지

모든 캐릭터 애니메이션을 무조건 한 번에 메모리에 올리지 않는다.

초기 후보 전략:

```text
현재 상태
→ 메모리에 유지

직전/자주 사용하는 상태
→ 작은 캐시

사용하지 않는 상태
→ 필요 시 로드
```

단, 디스크 I/O와 실제 메모리 소비를 측정한 뒤 캐시 전략을 결정한다.

### 측정

첫 프로토타입이 동작하면 다음을 기준선으로 기록한다.

- idle 메모리
- idle CPU
- walk 중 CPU
- 애니메이션 전환 시 메모리
- 타이머 실행 중 변화

임의의 메모리 목표치를 먼저 정하지 않고 실제 프로토타입 수치를 기준으로 회귀를 확인한다.

## 8. 구현 단계

### Phase 1. Native Window Prototype

목표:

- Rust 프로젝트 생성
- Win32 기본 창
- 투명 Layered Window
- 테스트 PNG 한 장 표시
- Always-on-top
- 정상 종료

완료 기준:

화면 위에 배경 없이 테스트 캐릭터가 보이고 프로그램을 정상 종료할 수 있다.

### Phase 1.5. Character Pack Foundation

목표:

- `assets/characters/` 기반 캐릭터 팩 구조
- `_template` 제공
- `character.toml` 파싱
- 모션 PNG 자동 검색
- 누락 모션 fallback
- 첫 실행 시 선택된 팩의 `idle` 첫 프레임 표시
- 캐릭터 팩 제작 문서와 이미지 생성 템플릿 제공

완료 기준:

`_template`을 복사하고 `idle` PNG 한 장만 넣어도 엔진 코드 수정 없이 실행할 수 있다.

### Phase 2. Input & Drag

목표:

- 캐릭터 클릭 판정
- 투명 영역 click-through
- 드래그 이동
- 화면 경계 처리

완료 기준:

캐릭터를 잡아 화면 안에서 자연스럽게 이동시킬 수 있다.

### Phase 3. Sprite Animation

목표:

- 프레임 애니메이션
- idle
- walk
- 좌우 방향
- 상태별 애니메이션 전환

완료 기준:

Pet이 idle과 walk 상태를 자연스럽게 전환하며 움직인다.

### Phase 4. Behavior Engine

목표:

- 상태 머신
- 행동별 허용 전이
- 가중치 기반 다음 행동 선택
- idle 시간에 따른 가중치 변화

완료 기준:

사용자 입력 없이도 캐릭터가 화면에서 자연스럽게 행동한다.

### Phase 5. Interaction Reactions

목표:

- click
- double click
- drag/drop
- happy / surprised 등 반응
- fall / land 상태

완료 기준:

사용자 입력에 따라 캐릭터 상태와 애니메이션이 달라진다.

### Phase 6. Timer

목표:

- Countdown Timer
- 트레이 메뉴
- 타이머 시작/취소
- 종료 알림
- Pet 상태와 연결

완료 기준:

25분 등의 타이머를 실행할 수 있고 Pet이 타이머 상태에 반응한다.

### Phase 7. Pomodoro & Settings

목표:

- Pomodoro
- 기본 설정 저장
- 캐릭터 설정
- 시작프로그램 옵션 검토

### Phase 8. Character Pack UX

목표:

- 트레이 메뉴에서 캐릭터 선택
- 마지막 선택 캐릭터 저장
- 캐릭터 팩 import/검증 흐름
- 잘못된 팩의 사용자 친화적 오류 표시
- 필요하면 미리보기 UI 제공

기본 캐릭터 팩 로딩 구조 자체는 Phase 1.5에서 먼저 구축한다.

## 9. 현재 비범위

초기에는 다음을 만들지 않는다.

- AI 대화 기능
- 음성 인식
- 클라우드 동기화
- 계정 시스템
- 복잡한 Todo 앱
- 자체 브라우저 UI
- 플러그인 마켓
- 네트워크 필수 기능
- 3D 캐릭터

필요성이 확인되기 전까지 핵심 Desktop Pet 경험에 집중한다.

## 10. 참고 프로젝트

### YijiaDuan/desktop-pet

https://github.com/YijiaDuan/desktop-pet

참고할 점:

- 상태 머신
- 클릭/더블클릭/호버/드래그
- 투명 영역 click-through
- 캐릭터 정의와 플랫폼 코드 분리

### Litrudy/DesktopPet

https://github.com/Litrudy/DesktopPet

참고할 점:

- PySide6 기반 상태 구조
- idle / walk / sleep / happy / dragged / returning
- 캐릭터 스프라이트 팩 구조
- 다중 모니터 처리

### OpenPets

https://github.com/OpenPetsHQ/openpets

참고할 점:

- Focus Timer
- Reminder
- 랜덤 행동
- Pet과 유틸리티 기능 연결
- 장기적인 확장 구조

그대로 베이스로 사용하지 않고 기능과 구조를 참고한다.

### bssm-oss/desktop-pet

https://github.com/bssm-oss/desktop-pet

참고할 점:

- Windows 네이티브 캐릭터 렌더링
- GIF/APNG/PNG sequence
- 투명 창
- 위치·크기 저장

### gauravs19/desktop-pets

https://github.com/gauravs19/desktop-pets

참고할 점:

- Weighted State Machine
- 행동 전이 가중치
- 여러 Pet을 하나의 simulation loop에서 관리하는 방식

### Shimeji 계열

대표 참고:

https://github.com/DalekCraft2/Shimeji-Desktop

참고할 점:

- 캐릭터 행동을 데이터로 정의하는 방식
- 오래된 Desktop Mascot 계열의 행동 설계

## 11. 다음 작업

Phase 1과 Phase 1.5 기반 작업을 마친 뒤 다음 개발 단계는 Phase 2 Input & Drag다.

예정 순서:

1. 캐릭터 알파값 기반 클릭 판정
2. 투명 픽셀 영역 click-through
3. 마우스 드래그 이동
4. 화면 밖으로 완전히 사라지지 않도록 경계 보정
5. 다중 모니터 좌표계 확인
6. 입력 처리 추가 전후의 메모리/유휴 CPU 비교

Phase 2에서는 아직 프레임 애니메이션이나 자동 이동을 넣지 않는다. 현재 캐릭터 팩의 `idle` 첫 프레임을 기준으로 입력과 드래그를 먼저 안정화한다.

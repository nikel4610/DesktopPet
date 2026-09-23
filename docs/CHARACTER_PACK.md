# Character Pack 가이드

Desktop Pet은 캐릭터 리소스를 엔진 코드와 분리해서 관리한다.

목표는 새 캐릭터를 추가할 때 Rust 코드를 수정하지 않고, 캐릭터 폴더를 복사한 뒤 PNG만 교체해도 실행할 수 있게 하는 것이다.

## 1. 가장 빠른 시작 방법

1. `assets/characters/_template/` 폴더를 복사한다.
2. 복사한 폴더 이름을 캐릭터 ID로 바꾼다.
3. `idle/0001.png`를 원하는 캐릭터 이미지로 교체한다.
4. 필요하면 `walk/`, `dragged/`, `happy/` 등에 추가 PNG를 넣는다.
5. `character.toml`의 이름, 크기, FPS 등을 조정한다.
6. Desktop Pet을 실행한다.

최소 실행 조건은 **idle PNG 1장**이다.

`_template`처럼 이름이 `_`로 시작하는 폴더는 실행 가능한 캐릭터 팩 검색에서 제외된다.

## 2. 권장 폴더 구조

```text
assets/
└─ characters/
   ├─ _template/
   │  ├─ character.toml
   │  ├─ preview.png
   │  ├─ idle/
   │  │  └─ 0001.png
   │  ├─ walk/
   │  ├─ dragged/
   │  ├─ happy/
   │  ├─ look/
   │  ├─ sleep/
   │  ├─ wake/
   │  ├─ sit/
   │  ├─ stretch/
   │  ├─ fall/
   │  ├─ land/
   │  └─ special/
   └─ my_character/
      ├─ character.toml
      ├─ preview.png
      └─ idle/
         └─ 0001.png
```

현재 엔진이 인식하는 모션 이름:

- `idle`
- `walk`
- `dragged`
- `happy`
- `look`
- `sleep`
- `wake`
- `sit`
- `stretch`
- `fall`
- `land`
- `special`

## 3. PNG 규칙

- PNG 형식을 사용한다.
- 투명 배경을 권장한다.
- 같은 모션의 모든 프레임은 같은 캔버스 크기를 사용한다.
- 캐릭터의 바닥 위치와 중심 위치를 프레임마다 최대한 일치시킨다.
- 파일명은 `0001.png`, `0002.png`, `0003.png`처럼 0을 채운 연속 번호를 권장한다.
- 엔진은 폴더 안의 PNG를 파일명 기준으로 정렬한다.
- PNG가 아닌 파일은 프레임 목록에서 무시한다.

현재는 프레임 경로만 먼저 스캔하고, 실제 표시할 이미지 파일만 디코딩한다. 모든 애니메이션을 한 번에 메모리에 올리지 않는다.

## 4. 필수/선택 모션

### 필수

- `idle`

`idle` 폴더에는 PNG가 최소 1장 있어야 한다.

### 권장

- `walk`
- `dragged`

### 선택

- `happy`
- `look`
- `sleep`
- `wake`
- `sit`
- `stretch`
- `fall`
- `land`
- `special`

선택 모션 폴더가 비어 있거나 존재하지 않아도 캐릭터 팩은 실행된다.

## 5. character.toml

기본 예시:

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

[dragged]
fps = 1
loop = true

[happy]
fps = 6
loop = false

[look]
fps = 4
loop = false

[fallbacks]
walk = "idle"
dragged = "idle"
happy = "idle"
look = "idle"
```

### 기본 정보

- `name`: 창에 사용할 캐릭터 표시 이름
- `author`: 캐릭터 팩 작성자
- `version`: 캐릭터 팩 버전
- `scale`: 원본 PNG 표시 배율. `1.0`은 원본 크기다.
- `anchor_x`, `anchor_y`: 이후 이동/낙하/바닥 정렬에 사용할 기준점
- `default_facing`: 기본 방향. 현재 허용값은 `left`, `right`

### 모션 설정

- `fps`: 애니메이션 재생 속도
- `loop`: 반복 여부
- `speed`: 이동형 모션에서 사용할 이동 속도. 현재 `walk` 이동에 적용한다.

모션 섹션이 없어도 해당 폴더에 PNG가 있으면 엔진 기본값으로 인식한다.

기본값 예:

- `walk`: 8 FPS, loop, speed 90
- `dragged`: 1 FPS, loop
- `happy`, `look`, `wake`, `land`, `special`: 6 FPS, non-loop
- 그 외: 4 FPS, loop

## 6. fallback

없는 모션을 요청했을 때 다른 모션으로 대신 재생할 수 있다.

예:

```toml
[fallbacks]
walk = "idle"
dragged = "idle"
happy = "idle"
look = "idle"
```

현재 로더는 다음 순서로 처리한다.

1. 요청한 모션에 실제 PNG가 있으면 그대로 사용
2. `fallbacks`에 대상이 있으면 해당 모션으로 이동
3. 별도 fallback이 없으면 자동으로 `idle` 사용
4. fallback 순환이 생기면 오류 처리

따라서 `idle`만 있어도 기본 실행이 가능하다.

## 7. 현재 캐릭터 선택 방식

현재 단계에서는 `assets/characters/` 아래에서 다음 조건을 만족하는 폴더를 이름순으로 찾아 첫 번째 팩을 사용한다.

- 디렉터리
- 이름이 `_`로 시작하지 않음
- `character.toml` 존재

개발/테스트용으로 캐릭터 루트 자체를 바꾸고 싶으면 환경 변수 `DESKTOP_PET_CHARACTER_ROOT`를 사용할 수 있다.

캐릭터 선택 UI와 트레이 메뉴는 이후 단계에서 추가한다.

## 8. 현재 구현 범위

현재 캐릭터 팩 로더는 다음을 담당한다.

- 캐릭터 팩 폴더 검색
- `character.toml` 파싱 및 기본 검증
- 모션 폴더별 PNG 프레임 목록 수집
- 누락 모션 fallback 해결
- 첫 실행 시 `idle` 첫 프레임 선택
- `scale` 적용

현재 `idle`·`walk` 프레임을 설정된 FPS와 반복 여부에 맞춰 재생한다. `walk` PNG가 없으면 `idle`로 대체해 이동한다. 다른 모션은 로드와 fallback만 지원하며, 행동 상태와의 연결은 이후 단계에서 구현한다.

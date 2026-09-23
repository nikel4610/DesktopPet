# DesktopPet

Rust와 Win32 API로 만드는 Windows 데스크톱 캐릭터 프로젝트입니다. 투명한 최상위 창에 캐릭터를 표시하고, 캐릭터 팩을 코드와 분리해 단계적으로 상호작용과 자율 행동을 추가합니다.

현재는 **프로토타입 단계**입니다. 배포용 실행 파일이나 설치 프로그램은 아직 제공하지 않습니다.

## 현재 구현 범위

- 투명 Layered Window에 선택된 캐릭터 팩의 PNG 표시와 투명 픽셀 입력 통과
- `character.toml` 설정과 모션별 PNG 경로 검색, 누락된 모션의 대체 처리
- 마우스 드래그와 커서가 있는 모니터 경계에 맞춘 창 위치 보정
- `idle`·`walk` 프레임 재생, 가중치 기반 자율 행동, 자동 이동과 화면 끝에서 좌우 방향 전환
- 클릭은 `happy`, 더블클릭은 `special`, 드래그는 `dragged`, 놓으면 `fall` → `land` 반응
- `assets/characters/_template` 캐릭터 팩 템플릿과 `default` 예제 팩

Windows 실기기에서 표시·투명 영역 입력 통과·드래그·2대 모니터 간 이동과 경계 보정·정상 종료를 확인했습니다. 색이 다른 프레임을 가진 임시 팩으로 `idle`·`walk` 재생과 방향 전환도 확인했습니다. 가중치 기반 행동과 클릭·더블클릭·드래그/놓기 입력을 적용한 뒤에도 창 표시·자동 이동·정상 종료를 확인했습니다. 기본 예제 팩에는 `idle` 이미지 한 장만 있어 이동이나 반응 중에도 같은 이미지가 표시됩니다. 이번 실행의 DPI는 96이었으며, 다른 DPI 배율은 아직 확인하지 않았습니다. CPU·메모리 측정값은 [PROJECT.md](PROJECT.md)에 기록했습니다. 타이머·포모도로와 트레이 메뉴는 아직 구현 전입니다.

## 실행과 테스트

Windows와 Rust 도구 체인이 필요합니다. 저장소 루트에서 실행합니다.

```powershell
cargo run --release
cargo test --offline
```

기본 실행은 `assets/characters/` 아래의 실행 가능한 팩을 이름순으로 검색해 첫 번째 팩을 선택합니다. 다른 캐릭터 루트를 지정하려면 실행 전에 환경 변수를 설정합니다.

```powershell
$env:DESKTOP_PET_CHARACTER_ROOT = "D:\path\to\characters"
cargo run --release
```

현재는 캐릭터 선택 UI가 없습니다. 창을 닫으면 프로그램이 종료됩니다.

## 캐릭터 팩 만들기

1. `assets/characters/_template/`을 새 이름으로 복사합니다. 밑줄(`_`)로 시작하는 폴더는 실행 팩 검색에서 제외됩니다.
2. `character.toml`의 이름과 표시 설정을 조정합니다.
3. 최소한 `idle/0001.png` 한 장을 넣습니다. 다른 모션의 PNG는 필요할 때 추가합니다.

프레임 규칙과 대체 모션 동작은 [캐릭터 팩 가이드](docs/CHARACTER_PACK.md), 이미지 제작 기준은 [이미지 생성 템플릿](docs/IMAGE_GEN_TEMPLATE.md)을 참고하세요. 현재 엔진은 `idle`·`walk` 프레임을 재생합니다. `walk` PNG가 없으면 `idle` 이미지를 사용하면서 이동합니다.

## 개발 환경

`src/`에 Win32 창, 입력 처리, 애니메이션, 캐릭터 설정·에셋 로더가 있습니다. `assets/characters/`는 캐릭터 데이터, `docs/`는 제작 안내입니다. 프로젝트 작업 기준은 [AGENTS.md](AGENTS.md)에 있습니다.

Codex에서 사용하는 선택적 프로젝트 설정은 `.codex/config.toml`에 있습니다. Ponytail 플러그인과 Serena 실행 파일은 각 개발 환경에 별도로 설치해야 합니다. Serena 프로젝트 설정은 `.serena/project.yml`에 있습니다. Codex가 프로젝트 설정을 읽으려면 이 저장소를 신뢰한 프로젝트로 열어야 합니다. 이 도구들은 프로그램 실행에 필요하지 않습니다.

작업 기록은 저장소의 별도 devlog 대신 Obsidian Vault의 일일 로그에 남깁니다.

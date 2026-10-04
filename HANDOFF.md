# DesktopPet 인수인계

기록일: 2026-10-04. 직전 정리 커밋은 `873d127`이며 `origin/main`에 푸시했다. 이 문서는 이후 담당자가 현재 상태를 확인하기 위한 기록이다.

## 사용자 결정과 최종 범위

사용자는 서서 눈 깜빡이기와 걸어다니기를 수용하고 이 상태에서 추가 개발을 종료했다. 손인사는 어깨·팔꿈치·손목 움직임이 부자연스러워 거절했으며 제작본·적용본·관련 계획을 삭제했다. 기지개·넥타이 조절·무작위 클릭 반응은 구현하지 않았다. 새 담당자가 사용자의 별도 요청을 받기 전에는 종료된 후속 계획을 자동으로 진행하지 않는다.

옵시디언 `00-Polaris/TODO.md`에서 DesktopPet의 재개 표 행과 작업 섹션을 삭제했다. 프로젝트 노트는 종료 상태를 기록했으며 이전 계획은 이력으로 남아 있다.

## 실제 실행 팩과 자료 위치

- 저장소: `D:\vscProject\DesktopPet`
- 최종 팩: `D:\vscProject\DesktopPet\local-assets\nate-animation-review\current-characters\nate-v7`
- 실행 루트: 위 팩의 부모인 `D:\vscProject\DesktopPet\local-assets\nate-animation-review\current-characters`
- PNG: 320×320, 투명 배경, scale=1.0, anchor=(160,308)
- idle: 4장, 유지 시간 `[6300,60,80,60]` ms. 몸·시선은 고정하고 눈만 깜빡인다.
- walk: 16장·16 FPS, 속도 90픽셀/초, 보행 그림 주기 약 1초. 한 번의 목표 걷는 시간은 약 1.4~3.38초다.
- 현재 팩에는 `idle`·`walk` 폴더만 있다. `happy`·`special`·`dragged`·`fall`·`land`는 idle로 대체한다. 입력 처리 자체는 기존 엔진에 남아 있다.
- 같은 실행 루트의 `_nate-v3`~`_nate-v6`는 이전 팩이다. 밑줄 폴더는 자동 선택에서 제외된다.

**최종 Nate 이미지와 제작 자료는 Git 제외 로컬 파일이다. GitHub만 clone하면 이 팩은 없다.** 다른 컴퓨터로 옮길 때는 위 로컬 팩을 별도로 복사해야 한다. 이번 인수인계에서도 이미지의 Git 제외 방침을 바꾸지 않았다. 기본 `assets/characters/default`는 Nate 최종본이 아니다.

최종 걷기 전체 시트는 `local-assets/nate-animation-review/walk16-frame14-join-fixed-full.png`, 재생 비교는 `walk16-frame14-join-fixed-preview.html`, 마지막 픽셀 수정 전후는 `frame14-pinholes-before-after.png`다. 이전 미리보기와 중간 후보가 여럿 있으므로 이름이 비슷한 파일을 최종본으로 착각하지 않는다. 현재 파일 무결성 기준은 `docs/FINAL_PACK_SHA256.txt`에 남긴다.

## 실행과 종료

Windows PowerShell에서 다음과 같이 실행한다. 환경 변수는 개별 팩 폴더가 아니라 팩들이 있는 부모 폴더를 지정한다.

```powershell
Set-Location 'D:\vscProject\DesktopPet'
$previousRoot = $env:DESKTOP_PET_CHARACTER_ROOT
try {
    $env:DESKTOP_PET_CHARACTER_ROOT = 'D:\vscProject\DesktopPet\local-assets\nate-animation-review\current-characters'
    cargo run --release --offline
} finally {
    $env:DESKTOP_PET_CHARACTER_ROOT = $previousRoot
}
```

종료 메뉴는 없다. 창 종료 경로를 사용할 수 없으면 작업 관리자에서 해당 `desktop-pet.exe`를 종료한다. 이전 실행의 PID나 `active-pet-pid.txt`는 재사용하지 말고 실제 프로세스를 확인한다. 종료 정리 후 Pet을 다시 실행하지 않았다.

## 코드와 검증 상태

- `src/main.rs`: Win32 창·입력·타이머·PNG 렌더링. 필요할 때 전체 이미지를 좌우 반전한다.
- `src/animation.rs`: 프레임 선택, idle의 프레임별 유지 시간, 이동·반응·idle 복귀.
- `src/behavior.rs`: 가중치 기반 idle·walk 행동과 반응 상태.
- `src/asset/character_pack.rs`, `src/config/character.rs`: 팩 로딩·설정·모션 대체.
- `src/image_limits.rs`: 이미지 크기·배율·디코더 할당 제한. 프레임당 64 MiB는 안전 상한이며 앱 메모리 목표가 아니다.

로더는 PNG 경로를 수집하고 실제 표시할 한 프레임을 디코딩한다. 모든 프레임 선로딩이나 캐시는 추가하지 않았다. 프레임 수가 늘었다고 전체 RGBA 용량만큼 상주 메모리가 증가하는 구조는 아니다. FPS 증가에 따른 디코딩·렌더링 비용은 별도로 측정해야 한다.

마지막 엔진 코드 변경 당시 테스트 31개·릴리스 빌드·엄격 Clippy·포맷 검사가 통과했다. 이후 변경은 로컬 이미지·팩 설정·문서였으며 종료 정리와 인수인계에서는 전체 코드 테스트를 다시 실행하지 않았다. v7 실행·창 핸들·짧은 자원 관찰 기록은 `PROJECT.md`와 `docs/ANIMATION_PLAN.md`에 있다. 단일 자원 표본으로 성능 우열이나 장기 안정성을 확정하지 않는다.

수용한 v7의 실제 입력·다중 모니터·혼합 DPI·장기 CPU/메모리 회귀는 모두 최종 검증된 상태가 아니다. 과거 다른 팩의 검증을 현재 팩의 검증처럼 보고하지 않는다. 새 코드 변경을 요청받으면 해당 범위에 맞춰 `cargo test --offline`, `cargo build --release --offline`, 필요한 실제 창 검증을 수행한다.

## 이미지 작업에서 발생한 문제

걷기 중간 프레임 전체 재생성은 몸이 흔들려 거절됐다. 원본 인접 프레임 사이의 다리 자세만 보강하고 나머지 픽셀을 보존해 16장으로 만들었다. 이후 6번·14번의 합성 잔상·경계와 14번의 투명 구멍 두 픽셀·가장자리 한 픽셀을 수정했고 사용자가 최종 결과를 수용했다. 최종 PNG를 초기 후보로 되돌리지 않는다.

손인사 실패에서는 원본 몸을 고정한 채 팔·소매만 조립한 결과 관절과 프레임 사이 자세의 일관성을 확보하지 못했다. 규격·해시·투명 테두리·연결 요소·엔진 프레임 순서 검사는 통과했지만 자연스러운 동작을 보증하지 못했다. 프로세스 응답·창 핸들 확인도 실제 애니메이션의 시각적 확인을 대신하지 못한다. 사용자가 실제 재생 결함을 지적한 뒤 적용을 철회했다.

향후 사용자가 이미지 수정을 다시 요청하면 실제 적용 팩에서 전체 스프라이트를 먼저 보여주고, 표시한 부분을 해당 PNG에서 픽셀 단위로 확대해 밝은/어두운 배경으로 비교한다. 수정 후 실제 적용 파일에서 시트를 다시 출력하고 재생도 확인한다. 확인하지 못한 품질은 확인했다고 말하지 않는다.

## 읽을 문서와 운영 기준

먼저 `AGENTS.md`, 이 문서, `PROJECT.md`의 현재 상태를 확인한다. 애니메이션 세부 이력은 `docs/ANIMATION_PLAN.md`, 팩 형식은 `docs/CHARACTER_PACK.md`를 참고한다. 커밋은 사용자가 요청할 때만 한다. 요청 밖 리팩터링·의존성 추가·후속 기능 개발은 하지 않는다.

Vault 프로젝트 노트는 `D:\Obsidian_Vault\Cortex\20-Areas\Coding\DesktopPet.md`, 종료·실패·검증 이력은 `D:\Obsidian_Vault\Cortex\60-Logs\2026-10-04.md`에 있다. Vault를 수정할 때는 먼저 해당 Vault의 `00-Polaris/LLM_Instructions.md`를 따른다.

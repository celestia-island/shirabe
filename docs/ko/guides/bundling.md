# 네이티브 라이브러리 번들링

shirabe로 구축된 제품을 배포할 때 일반적으로 두 종류의 네이티브 파일이 바이너리와 함께 포함되어야 합니다:

1. **브라우저 백엔드의 런타임 의존성.** 가져온 Chrome for Testing 빌드는 클린 컨테이너에 없을 수 있는 시스템 라이브러리(`libnss3.so`, `libdbus-1.so`, …)에 링크됩니다.
2. **자체 네이티브 의존성** — 크레이트가 링크하는 `.so` / `.dylib` / `.dll` 파일.

shirabe는 패키저에게 하나의 모듈 — `shirabe::bundle` — 을 제공하여 이 두 가지를 모두 처리합니다.

## 포함할 항목 선언하기

`SHIRABE_BUNDLE_LIBS`를 사용하여 파일을 그대로 나열합니다 (경로 구분자 목록: Unix에서는 `:`, Windows에서는 `;`):

```bash
SHIRABE_BUNDLE_LIBS="/opt/myapp/libfoo.so:/opt/myapp/libbar.so"
```

또는 `bundle.toml` 매니페스트를 작성하고 `SHIRABE_BUNDLE_MANIFEST`로 가리킵니다:

```toml
[[lib]]
path = "third_party/libfoo.so"
optional = true
target_os = "linux"

[[lib]]
path = "third_party/foo.dll"
```

두 소스는 `BundleSpec::from_env()`에 의해 병합됩니다.

## 포함할 항목 발견하기

`collect_runtime_deps(exe)`는 바이너리에서 공유 라이브러리 의존성을 스캔합니다 — Linux에서는 `ldd`, macOS에서는 `otool -L`, Windows에서는 최선의 PE 임포트 스캔 — 그리고 기록된 각 의존성을 리졸버가 찾은 위치와 함께 반환합니다.

## 모두 합치기

`BundleReport::build(&backend_exe)`는 선언된 번들과 해결된 백엔드 실행 파일에서 발견된 의존성을 병합하고, `render_bundle_report(&report)`는 이를 릴리스 스크립트가 출력하거나 매니페스트에 기록할 수 있는 사람이 읽을 수 있는 가이드로 변환합니다:

```rust
use shirabe::{BundleReport, render_bundle_report};

let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

그런 다음 릴리스 스크립트는 모든 `resolved` 경로(및 선언된 모든 비선택적 라이브러리)를 배포 디렉터리로 `cp`하여 Chrome이나 해당 시스템 라이브러리가 설치되지 않은 머신에서도 실행되는 자체 완결형 제품을 생성할 수 있습니다.

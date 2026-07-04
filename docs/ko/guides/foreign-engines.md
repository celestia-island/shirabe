# 외부 엔진 — Firefox & Servo

Chromium 계열(Chrome / Chromium / Edge)은 shirabe 자체 CDP 엔진을 통해 프로세스
내에서 구동됩니다. **Firefox**와 **Servo**는 다른 경로를 택합니다: 코어가 너무
거대하기 때문에, 브라우저 벤더(또는 해당 코어를 빌드하는 누구든)가 고정된 C ABI에
맞춰 작은 어댑터를 컴파일하고 동적 라이브러리로 제공하도록 합니다 —
[ort](https://crates.io/crates/ort)가 ONNX Runtime에 사용하는 것과 동일한
모델입니다. shirabe는 "얇은 C 바인딩 래퍼"로서, 벤더 라이브러리를 dlopen하고
범용 [`Engine`](https://shirabe.docs.celestia.world) 트레잇을 통해 호출을 중계
합니다.

```
your app ── shirabe (CDP engine) ── Chrome / Chromium / Edge   (in-process)
        └─ shirabe (FFI wrapper) ── libshirabe_engine_firefox ── Firefox core
                                 └ libshirabe_engine_servo   ── Servo core
```

## 활성화

```toml
shirabe = { version = "0.1", features = ["foreign-engine"] }
```

그런 다음 외부 백엔드를 선택합니다:

```bash
SHIRABE_BACKEND=firefox shirabe debug --port 3001
SHIRABE_BACKEND=servo   shirabe debug --port 3001
```

## 벤더 라이브러리가 내보내는 C ABI

```c
typedef struct shirabe_engine shirabe_engine;

shirabe_engine *shirabe_engine_new(const char *options_json);   /* JSON opts   */
void  shirabe_engine_destroy(shirabe_engine *eng);
int   shirabe_engine_navigate(shirabe_engine *eng, const char *url);
char *shirabe_engine_evaluate(shirabe_engine *eng, const char *js); /* JSON out */
void  shirabe_engine_free_string(shirabe_engine *eng, char *s);
int   shirabe_engine_screenshot(shirabe_engine *eng,
                                unsigned char **out, size_t *out_len);   /* PNG */
void  shirabe_engine_free_pixels(shirabe_engine *eng, unsigned char *buf, size_t len);
const char *shirabe_engine_id(void);                              /* "firefox" … */
```

이 ABI에 맞춘 수백 줄의 어댑터 코드만으로도 브라우저 코어 전체를 구동하기에
충분합니다. shirabe가 HTTP를 통해 노출하는 모든 기능은 이 다섯 가지 연산 위에
구축되어 있습니다.

## 라이브러리 출처

`CdylibEngine::open`은 다음 위치에서 `libshirabe_engine_<id>.{so,dylib,dll}`을
찾습니다:

1. `SHIRABE_ENGINE_PATH` — 명시적 재정의.
2. 현재 실행 파일 옆.
3. `<cache>/shirabe/engines/<id>/` — 릴리스 가져오기 단계에서 다운로드한 복사본을
   배치하는 곳(릴리스 워크플로가 사전 빌드된 라이브러리를 자체 태그로 GitHub
   Releases에 게시합니다).

벤더가 라이브러리를 게시하기 전까지는 Firefox/Servo를 선택하면 FFI 계약을
가리키는 명확한 오류가 반환됩니다 — shirabe는 `firefox`를 CDP를 말하는 것처럼
실행하려 하지 않습니다.

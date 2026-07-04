# 外部引擎 — Firefox & Servo

Chromium 家族（Chrome / Chromium / Edge）透過 shirabe 自有的 CDP 引擎在程序內
驅動。**Firefox** 和 **Servo** 走另一條路：它們的核心非常龐大，因此我們讓瀏覽器
廠商（或任何建構這些核心的人）針對固定的 C ABI 編譯一個小型適配器，並以動態庫
形式發佈——與 [ort](https://crates.io/crates/ort) 用於 ONNX Runtime 的模型相同。
shirabe 是「薄 C 綁定包裝器」：它 dlopen 廠商庫，並通過泛型
[`Engine`](https://shirabe.docs.celestia.world) trait 路由調用。

```
your app ── shirabe (CDP engine) ── Chrome / Chromium / Edge   (in-process)
        └─ shirabe (FFI wrapper) ── libshirabe_engine_firefox ── Firefox core
                                 └ libshirabe_engine_servo   ── Servo core
```

## 啟用

```toml
shirabe = { version = "0.1", features = ["foreign-engine"] }
```

然後選擇一個外部後端：

```bash
SHIRABE_BACKEND=firefox shirabe debug --port 3001
SHIRABE_BACKEND=servo   shirabe debug --port 3001
```

## 廠商庫匯出的 C ABI

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

針對此 ABI 的幾百行適配器代碼就足以驅動整個瀏覽器核心；shirabe 通過 HTTP 暴露
的所有功能都建立在這五個操作之上。

## 庫的來源

`CdylibEngine::open` 在以下位置查找 `libshirabe_engine_<id>.{so,dylib,dll}`：

1. `SHIRABE_ENGINE_PATH` — 顯式覆蓋。
2. 當前可執行檔旁邊。
3. `<cache>/shirabe/engines/<id>/` — 發佈獲取步驟放置下載副本的位置（發佈工作流
   將預建構的庫發佈到 GitHub Releases 上，使用各自的標籤）。

在廠商發佈庫之前，選擇 Firefox/Servo 會返回一個清晰的錯誤，指向 FFI 契約 —
shirabe 絕不會試圖像對待 CDP 一樣去啟動 `firefox`。

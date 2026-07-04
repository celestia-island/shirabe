# 外部引擎 — Firefox & Servo

Chromium 家族（Chrome / Chromium / Edge）通过 shirabe 自有的 CDP 引擎在进程内
驱动。**Firefox** 和 **Servo** 走另一条路：它们的核心非常庞大，因此我们让浏览器
厂商（或任何构建这些核心的人）针对固定的 C ABI 编译一个小型适配器，并以动态库
形式发布——与 [ort](https://crates.io/crates/ort) 用于 ONNX Runtime 的模型相同。
shirabe 是"薄 C 绑定包装器"：它 dlopen 厂商库，并通过泛型
[`Engine`](https://shirabe.docs.celestia.world) trait 路由调用。

```
your app ── shirabe (CDP engine) ── Chrome / Chromium / Edge   (in-process)
        └─ shirabe (FFI wrapper) ── libshirabe_engine_firefox ── Firefox core
                                 └ libshirabe_engine_servo   ── Servo core
```

## 启用

```toml
shirabe = { version = "0.1", features = ["foreign-engine"] }
```

然后选择一个外部后端：

```bash
SHIRABE_BACKEND=firefox shirabe debug --port 3001
SHIRABE_BACKEND=servo   shirabe debug --port 3001
```

## 厂商库导出的 C ABI

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

针对此 ABI 的几百行适配器代码就足以驱动整个浏览器核心；shirabe 通过 HTTP 暴露
的所有功能都建立在这五个操作之上。

## 库的来源

`CdylibEngine::open` 在以下位置查找 `libshirabe_engine_<id>.{so,dylib,dll}`：

1. `SHIRABE_ENGINE_PATH` — 显式覆盖。
2. 当前可执行文件旁边。
3. `<cache>/shirabe/engines/<id>/` — 发布获取步骤放置下载副本的位置（发布工作流
   将预构建的库发布到 GitHub Releases 上，使用各自的标签）。

在厂商发布库之前，选择 Firefox/Servo 会返回一个清晰的错误，指向 FFI 契约 —
shirabe 绝不会试图像对待 CDP 一样去启动 `firefox`。

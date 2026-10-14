//! 演示 `client.http()`：用可失败的链式构造器发一个 GET，并读回状态码与响应头。
//!
//! 编译验证：`cargo check --example http`
//! 运行需要真实 Steam 客户端（HTTP 走 Steam 的 relay，不是裸 socket）；
//! 本例只完成编译验证，未运行验证。
//!
//! 关键语义：
//! - 链上每一步都返回 `Result<Self, SteamError>`：含内嵌 NUL 的字符串降级为
//!   `InvalidParameter` 而不是 panic，写错 URL 在构造期就能拿到错误；
//! - `send()` 的回调拿到的是 `HttpResponse`，状态码在回调里就已经复制进结构体，
//!   不依赖 Steam 句柄仍然存活；
//! - `HttpResponse` / `CookieContainer` 都是 RAII，drop 即释放 Steam 侧句柄。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use steamworks::{Client, HttpMethod};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::init()?;
    let http = client.http();

    // 显式走链式构造，演示每一步都可能判红；简单 GET 也可以直接 http.get(..)
    let done = Arc::new(AtomicBool::new(false));
    {
        let done = Arc::clone(&done);
        http.request(
            HttpMethod::Get,
            "https://api.steampowered.com/ISteamWebAPIUtil/GetServerInfo/v1/",
        )?
        .user_agent("steamworks-rs example")?
        .absolute_timeout(10_000)?
        .send(move |response| {
            // 回调拿到的是 Result：Steam 侧请求失败（超时、DNS、句柄失效）
            // 会走 Err 分支，而不是给一个零值响应让调用方误判。
            match response {
                Ok(response) => {
                    println!("status={}", response.status_code());
                    if let Ok(Some(server)) = response.header("server") {
                        println!("server={server}");
                    }
                    match response.body() {
                        Ok(bytes) => println!("body={} bytes", bytes.len()),
                        Err(e) => println!("could not read body: {e}"),
                    }
                }
                Err(e) => println!("request failed: {e}"),
            }
            done.store(true, Ordering::SeqCst);
        })?;
    }

    // 回调由 Steam 投递，必须靠主循环推进；这里等到响应到达为止。
    while !done.load(Ordering::SeqCst) {
        client.run_callbacks();
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    Ok(())
}

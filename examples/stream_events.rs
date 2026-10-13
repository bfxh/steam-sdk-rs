//! 演示 `CallbackStream`：把 Steam 事件消费接入 `futures::Stream` 组合链。
//!
//! 编译验证：`cargo build --features futures --example stream_events`
//! 运行需要真实 Steam 客户端；本文所述模型仅完成编译验证，未运行验证。
//!
//! 关键语义：
//! - Steam 回调由 Steam 客户端线程投递，`Stream` 无法「阻塞等待」它；
//! - 游戏主循环仍是事件驱动源：主循环侧调用 `pump()` 推进队列；
//! - `Stream` 消费者只负责取走已到达的事件。
//!
//! 为什么这里用 `now_or_never()` 而不是 `block_on`：
//! `CallbackStream` 在队列空时返回 `Poll::Pending` 并登记 waker，
//! 而 `wake()` 只能由主循环的 `pump()` 触发。若主循环线程自己
//! 阻塞在 `block_on` 里，waker 永远无人触发 → 首个空队列即死锁。
//! `now_or_never()` 做一次非阻塞 poll，拿不到事件就立即继续推进主循环。

use futures::FutureExt;
use futures::StreamExt;
use std::thread;
use std::time::Duration;
use steamworks::{CallbackStream, Client};

fn main() {
    let client = Client::init().expect("steam client must be running");
    let mut stream = CallbackStream::new(client.clone());

    for _ in 0..10 {
        // 主循环推进 Steam 事件队列（等价于定期 process_callbacks）
        stream.pump();

        // 非阻塞地取一个已到达的事件；队列空则立即返回 None，不挂起
        match stream.next().now_or_never() {
            Some(Some(ev)) => println!("steam event: {:?}", ev),
            Some(None) => println!("stream ended (unreachable for CallbackStream)"),
            None => println!("(no event yet)"),
        }

        thread::sleep(Duration::from_millis(100));
    }
}

use crate::{CallbackResult, Client};
use futures::stream::Stream;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

/// 把 steamworks-rs 已有的「主循环 + process_callbacks」模型暴露成
/// 一个 `futures::Stream`，供需要异步的用户按自己的运行时消费 Steam 事件。
///
/// 设计约束（刻意为之）：
/// - **不**绑定 `tokio` / `async-std` / 任何具体运行时，仅依赖 `futures::Stream` trait；
/// - **不**承诺「等待 Steam 事件到达」——Steam 回调由 Steam 客户端驱动，
///   `poll_next` 只能推进事件队列并搬运已到达的事件；
/// - 没有事件时返回 `Poll::Pending`，这意味着调用方必须另有地方
///   持续产生 `wake`（最常见做法是在游戏主循环里周期性 `wake()`）。
///
/// # 何时用它
///
/// - 需要把 Steam 事件接进已有的 `Stream`/`Future` 组合链（`.map` / `.filter`）；
/// - 想在非阻塞消费循环里逐个取事件，而不注册 `register_callback`。
///
/// # 何时不用
///
/// - 游戏主循环里已有的 `process_callbacks(|ev| ...)` 是更直接、
///   也更符合 Steam 事件语义的做法，优先使用它。
pub struct CallbackStream {
    client: Client,
    /// 已到达、但尚未被 `poll_next` 取走的事件。
    /// `process_callbacks` 一次可能搬运多个事件，必须排队而非只留最后一个。
    pending: VecDeque<CallbackResult>,
    /// 最近一次 `poll_next` 返回 `Pending` 后留下的唤醒句柄。
    /// 由持有主循环的一侧在推进 Steam 事件后调用以重新唤醒该任务。
    waker: Option<std::task::Waker>,
}

impl CallbackStream {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            pending: VecDeque::new(),
            waker: None,
        }
    }

    /// 推进一次 Steam 事件队列并缓冲新到达的事件。
    ///
    /// 若此前 `poll_next` 返回过 `Pending`，搬运到事件（或本调用）会唤醒
    /// 等待方。主循环可以在调用 `process_callbacks` 后等价地调用本方法，
    /// 使 `Stream` 消费者跟上事件流。
    pub fn pump(&mut self) {
        let mut fetched = Vec::new();
        self.client.process_callbacks(|ev| fetched.push(ev));
        let had_new = !fetched.is_empty();
        self.pending.extend(fetched);
        if had_new {
            if let Some(waker) = self.waker.take() {
                waker.wake();
            }
        }
    }
}

impl Stream for CallbackStream {
    type Item = CallbackResult;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(ev) = this.pending.pop_front() {
            return Poll::Ready(Some(ev));
        }

        // 队列空：推进一次 Steam 事件，看是否有新事件立刻到达。
        this.pump();
        if let Some(ev) = this.pending.pop_front() {
            return Poll::Ready(Some(ev));
        }

        // Steam 回调无法在此处阻塞等待（它由 Steam 客户端线程投递），
        // 记下 waker 后返回 Pending，由主循环侧 pump() 唤醒。
        this.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

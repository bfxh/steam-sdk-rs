//! 演示 `client.inventory()`：拉取全量库存并打印每件物品的定义与数量。
//!
//! 编译验证：`cargo check --example inventory`
//! 运行需要真实 Steam 客户端 + 已在 Steamworks 后台配置好的物品定义；
//! 本例只完成编译验证，未运行验证。
//!
//! 关键语义：
//! - `get_all_items()` 返回的是**异步查询的句柄包装**（`InventoryResult`），
//!   拿到它不等于拿到数据；
//! - 数据就绪由 `InventoryResultReady` 回调通知，轮询侧用 `is_ready()`；
//! - `InventoryResult` 在 drop 时销毁 Steam 句柄，所以不要把它留在栈上不管，
//!   也不要提前 `into_raw()`。

use std::sync::{Arc, Mutex};
use steamworks::{Client, InventoryResult, InventoryResultReady};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::init()?;

    // 回调只记录「哪个句柄就绪了」，真正把数据取走发生在主循环里：
    // 回调闭包必须是 'static + Send，把 InventoryResult 搬进去会让它的
    // 生命周期和 Steam 回调线程绑死。
    let ready = Arc::new(Mutex::new(Vec::<i32>::new()));
    {
        let ready = Arc::clone(&ready);
        client.register_callback(move |ev: InventoryResultReady| {
            ready.lock().unwrap().push(ev.handle);
            match ev.result {
                Ok(()) => println!("result {} ready", ev.handle),
                Err(e) => println!("result {} failed: {e}", ev.handle),
            }
        });
    }

    let inventory = client.inventory();
    let result: InventoryResult = inventory.get_all_items()?;
    println!("requested full inventory, handle={}", result.raw());

    // 主循环：推进回调，直到查询就绪。真实游戏里这段就是每帧一次。
    loop {
        client.run_callbacks();
        if result.is_ready() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    if let Err(e) = result.status() {
        return Err(Box::new(e));
    }

    for (index, item) in result.items()?.iter().enumerate() {
        let u32_index = u32::try_from(index)?;
        let name = result
            .item_property(u32_index, "item_name")?
            .unwrap_or_else(|| "<untitled>".to_string());
        println!(
            "{name}: {} item(s), def={}, instance={}, removed={}",
            item.quantity,
            item.definition.raw(),
            item.item_id.raw(),
            if item.is_removed() { "yes" } else { "no" },
        );
    }
    Ok(())
}

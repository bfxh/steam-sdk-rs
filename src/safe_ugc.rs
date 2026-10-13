//! 可失败的 UGC 更新链式构建器。
//!
//! 上游 `UpdateHandle`（[`crate::UpdateHandle`]）的每个 setter 在遇到
//! 非法输入时直接 `panic`：路径不存在时 `canonicalize().unwrap()`，
//! 字符串含空字节时 `CString::new(..).unwrap()`。库的 panic 会绕过调用方
//! 的错误处理路径，且在多线程游戏主循环里意味着进程崩溃。
//!
//! `SafeUgcUpdate` 包装同一个 Steam 更新句柄，把每一步的失败收敛为
//! `Result<Self, SafeUgcError>`，让调用方可以 `?` 传播或显式处理：
//!
//! ```ignore
//! use steamworks::SafeUgcError;
//!
//! fn upload(client: &steamworks::Client, app: steamworks::AppId,
//!           file: steamworks::PublishedFileId) -> Result<(), SafeUgcError> {
//!     client.ugc()
//!         .safe_item_update(app, file)
//!         .title("My Mod")?
//!         .description("A cool mod")?
//!         .content_path(std::path::Path::new("./content"))?
//!         .visibility(steamworks::remote_storage::PublishedFileVisibility::Public)
//!         .submit(None, |r| println!("{r:?}"));
//!     Ok(())
//! }
//! ```
//!
//! 设计取舍：本类型不重新实现 Steam API 调用，而是复用上游 `UpdateHandle`
//! 的既有行为（含其对无效句柄的 `assert!`），只在其之上增加输入校验层。
//! 因此上游未来的行为改进会自动传导到这里。

use super::*;

use std::error::Error;
use std::fmt;
use std::path::Path;

/// `SafeUgcUpdate` 构造/设置阶段的失败原因。
///
/// 全部变体都对应上游会 `panic` 的输入条件，这里改为可恢复的错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafeUgcError {
    /// 路径无法 canonicalize（不存在、无权限、或路径含非法字符）。
    InvalidPath(std::path::PathBuf),
    /// 字符串含空字节，无法转换为 C 字符串传给 Steam API。
    InteriorNul(&'static str),
}

impl fmt::Display for SafeUgcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SafeUgcError::InvalidPath(p) => {
                write!(
                    f,
                    "path does not exist or is not accessible: {}",
                    p.display()
                )
            }
            SafeUgcError::InteriorNul(field) => {
                write!(f, "value for `{field}` contains an interior nul byte")
            }
        }
    }
}

impl Error for SafeUgcError {}

/// 可失败的 UGC 更新链式构建器，替代 [`crate::UpdateHandle`] 的 panic 版 setter。
///
/// 通过 [`crate::UGC::safe_item_update`] 创建。每个 setter 返回
/// `Result<Self, SafeUgcError>`，校验失败时不触碰 Steam API、不 panic。
pub struct SafeUgcUpdate {
    inner: UpdateHandle,
}

impl SafeUgcUpdate {
    pub(crate) fn new(inner: UpdateHandle) -> Self {
        Self { inner }
    }

    /// 校验字符串可安全转为 C 字符串，返回其长度（供未来可能的零拷贝优化）。
    fn check_cstr(field: &'static str, s: &str) -> Result<(), SafeUgcError> {
        if s.as_bytes().contains(&0) {
            Err(SafeUgcError::InteriorNul(field))
        } else {
            Ok(())
        }
    }

    /// 设置标题。字符串含空字节时返回 `Err`。
    pub fn title(self, title: &str) -> Result<Self, SafeUgcError> {
        Self::check_cstr("title", title)?;
        Ok(Self {
            inner: self.inner.title(title),
        })
    }

    /// 设置描述。字符串含空字节时返回 `Err`。
    pub fn description(self, description: &str) -> Result<Self, SafeUgcError> {
        Self::check_cstr("description", description)?;
        Ok(Self {
            inner: self.inner.description(description),
        })
    }

    /// 设置内容目录。路径不存在时返回 `Err`（上游此处 `canonicalize().unwrap()`）。
    pub fn content_path(self, path: &Path) -> Result<Self, SafeUgcError> {
        path.canonicalize()
            .map_err(|_| SafeUgcError::InvalidPath(path.to_path_buf()))?;
        Ok(Self {
            inner: self.inner.content_path(path),
        })
    }

    /// 设置预览文件路径。路径不存在时返回 `Err`。
    pub fn preview_path(self, path: &Path) -> Result<Self, SafeUgcError> {
        path.canonicalize()
            .map_err(|_| SafeUgcError::InvalidPath(path.to_path_buf()))?;
        Ok(Self {
            inner: self.inner.preview_path(path),
        })
    }

    /// 设置元数据。字符串含空字节时返回 `Err`。
    pub fn metadata(self, metadata: &str) -> Result<Self, SafeUgcError> {
        Self::check_cstr("metadata", metadata)?;
        Ok(Self {
            inner: self.inner.metadata(metadata),
        })
    }

    /// 设置可见性。纯枚举映射，无失败路径。
    pub fn visibility(self, visibility: remote_storage::PublishedFileVisibility) -> Self {
        Self {
            inner: self.inner.visibility(visibility),
        }
    }

    /// 设置标签。任一标签含空字节时返回 `Err`。
    pub fn tags<S: AsRef<str>>(
        self,
        tags: Vec<S>,
        allow_admin_tags: bool,
    ) -> Result<Self, SafeUgcError> {
        for t in &tags {
            Self::check_cstr("tag", t.as_ref())?;
        }
        Ok(Self {
            inner: self.inner.tags(tags, allow_admin_tags),
        })
    }

    /// 添加键值标签。键或值含空字节时返回 `Err`。
    pub fn add_key_value_tag(self, key: &str, value: &str) -> Result<Self, SafeUgcError> {
        Self::check_cstr("key_value_tag.key", key)?;
        Self::check_cstr("key_value_tag.value", value)?;
        Ok(Self {
            inner: self.inner.add_key_value_tag(key, value),
        })
    }

    /// 消耗构建器并提交更新。
    ///
    /// 语义与上游 [`crate::UpdateHandle::submit`] 完全一致：注册回调并
    /// 立即返回 `UpdateWatchHandle`。此方法本身不返回 `Result`——提交是
    /// 异步的，其结果通过 `cb` 回调传递。
    pub fn submit<F>(self, change_note: Option<&str>, cb: F) -> UpdateWatchHandle
    where
        F: FnOnce(Result<(PublishedFileId, bool), SteamError>) + 'static + Send,
    {
        self.inner.submit(change_note, cb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_cstr_accepts_plain_text() {
        assert_eq!(SafeUgcUpdate::check_cstr("f", "hello"), Ok(()));
    }

    #[test]
    fn check_cstr_accepts_empty() {
        assert_eq!(SafeUgcUpdate::check_cstr("f", ""), Ok(()));
    }

    #[test]
    fn check_cstr_rejects_interior_nul() {
        assert_eq!(
            SafeUgcUpdate::check_cstr("title", "bad\0title"),
            Err(SafeUgcError::InteriorNul("title"))
        );
    }

    #[test]
    fn check_cstr_rejects_trailing_nul() {
        // CString::new 也拒绝结尾 nul（它隐式添加终止符），校验层必须一并拦下。
        assert_eq!(
            SafeUgcUpdate::check_cstr("f", "trailing\0"),
            Err(SafeUgcError::InteriorNul("f"))
        );
    }

    #[test]
    fn error_display_includes_path() {
        let e = SafeUgcError::InvalidPath(std::path::PathBuf::from("/no/such/dir"));
        assert!(e.to_string().contains("/no/such/dir"));
    }

    #[test]
    fn error_display_includes_field() {
        let e = SafeUgcError::InteriorNul("description");
        assert!(e.to_string().contains("description"));
    }
}

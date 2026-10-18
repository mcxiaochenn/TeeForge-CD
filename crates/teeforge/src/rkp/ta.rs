//! 高通 KeymasterUtils 厂商 ABI 适配 [Qualcomm KeymasterUtils vendor ABI]
//!
//! 移植自 Duck-ToolBox 的 `duck-rkp-device::keymint`（MIT）。
//! Ported from Duck-ToolBox `duck-rkp-device::keymint` (MIT).
//!
//! 对象布局与符号随固件变化，存在性检查不能证明 ABI 兼容；调用一律在隔离子进程
//! 中进行，崩溃与厂商输出不会污染主进程。
//! Layout and symbols vary by firmware; calls always run in an isolated child.
//!
//! 本层只负责调用，**不解释响应**：返回原始缓冲区，帧布局由 `protocol::decode_frame`
//! 结合业务结构判定。早期实现在这里剥帧、上层又剥一次，导致 CBOR 被当成帧头解析。
//! This layer does not interpret the reply; framing is resolved exactly once upstream.
//!
//! 探测模式 [`PROBES`] 用于区分「固件不认命令」「缓冲区约定不同」「参数顺序相反」：
//! 每个变体在独立子进程里执行，崩溃只会丢失该组结果。
//! Probe variants run one per child process, so a crash only loses that variant.

use crate::error::Result;
use crate::rkp::protocol::BUFFER_SIZE;

/// 原始响应缓冲区与 `send_cmd` 返回码。
///
/// 保留整块缓冲区（而不是解析后的 payload），是为了让上层能在不同帧布局之间尝试，
/// 并在全部失败时把首字节 hex 透出给诊断界面。
/// The whole buffer is kept so the caller can try several layouts and report raw bytes.
#[derive(Debug, Clone)]
pub(crate) struct RawFrame {
    pub(crate) bytes: Vec<u8>,
    pub(crate) status: i32,
}

/// 响应缓冲区的初始化方式。
///
/// 部分固件会把 out 缓冲区的前几个字节当作「容量」或状态字段读取，预置哨兵反而会
/// 让它走空响应分支，因此探测时需要区分这三种做法。
/// Some firmware reads the first bytes of the out buffer as capacity or status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BufferInit {
    /// 前 4 字节写 `i32::MIN`，用于识别「完全未写入」。
    Sentinel,
    /// 全 0。
    Zero,
    /// 前 4 字节写缓冲区容量。
    Capacity,
}

/// 请求帧的封装方式。
///
/// 主机侧只验证协议与帧构造，厂商调用路径仅在 arm64 Android 编译，
/// 因此非该目标下 `LengthLe` 可能是未使用变体。
#[cfg_attr(
    not(all(target_os = "android", target_arch = "aarch64")),
    allow(dead_code)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestFrame {
    /// `[u32 LE cmd][cbor]`，当前实现。
    Plain,
    /// `[u32 LE cmd][u32 BE len][cbor]`
    LengthBe,
    /// `[u32 LE cmd][u32 LE len][cbor]`
    ///
    /// 保留为候选：大端约定更常见，但小端实现存在，需要时加一行变体即可。
    /// Kept as a candidate; only big-endian is currently probed.
    #[allow(dead_code)]
    LengthLe,
}

/// 一次调用的全部可变参数。
#[cfg_attr(
    not(all(target_os = "android", target_arch = "aarch64")),
    allow(dead_code)
)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct CallOptions {
    pub(crate) level: i32,
    pub(crate) buffer: BufferInit,
    pub(crate) buffer_len: usize,
    pub(crate) frame: RequestFrame,
}

impl Default for CallOptions {
    fn default() -> Self {
        Self {
            level: 1,
            buffer: BufferInit::Sentinel,
            buffer_len: BUFFER_SIZE,
            frame: RequestFrame::Plain,
        }
    }
}

/// 一次调用的原始观察，包含请求缓冲区被改写的情况。
pub(crate) struct CallResult {
    pub(crate) status: i32,
    pub(crate) response: Vec<u8>,
    pub(crate) request_before: Vec<u8>,
    pub(crate) request_after: Vec<u8>,
}

/// 探测变体表：标签 + 参数。
///
/// 顺序固定，`--rkp-probe` 的输出按这个顺序排列，便于与现场日志对照。
pub(crate) const PROBES: [(&str, CallOptions); 6] = [
    (
        "baseline",
        CallOptions {
            level: 1,
            buffer: BufferInit::Sentinel,
            buffer_len: BUFFER_SIZE,
            frame: RequestFrame::Plain,
        },
    ),
    (
        "zero-buffer",
        CallOptions {
            level: 1,
            buffer: BufferInit::Zero,
            buffer_len: BUFFER_SIZE,
            frame: RequestFrame::Plain,
        },
    ),
    (
        "capacity-header",
        CallOptions {
            level: 1,
            buffer: BufferInit::Capacity,
            buffer_len: BUFFER_SIZE,
            frame: RequestFrame::Plain,
        },
    ),
    (
        "small-buffer",
        CallOptions {
            level: 1,
            buffer: BufferInit::Zero,
            buffer_len: 0x2000,
            frame: RequestFrame::Plain,
        },
    ),
    (
        "level-0",
        CallOptions {
            level: 0,
            buffer: BufferInit::Zero,
            buffer_len: BUFFER_SIZE,
            frame: RequestFrame::Plain,
        },
    ),
    (
        "length-prefix",
        CallOptions {
            level: 1,
            buffer: BufferInit::Zero,
            buffer_len: BUFFER_SIZE,
            frame: RequestFrame::LengthBe,
        },
    ),
];

/// 按变体把命令头与 payload 重新封装成实际发送的字节。
#[cfg_attr(
    not(all(target_os = "android", target_arch = "aarch64")),
    allow(dead_code)
)]
pub(crate) fn frame_request(request: &[u8], frame: RequestFrame) -> Vec<u8> {
    let Some((command, payload)) = request.split_at_checked(4) else {
        return request.to_vec();
    };
    let length = payload.len().to_le_bytes();
    match frame {
        RequestFrame::Plain => request.to_vec(),
        RequestFrame::LengthBe => {
            let mut out = command.to_vec();
            out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            out.extend_from_slice(payload);
            out
        }
        RequestFrame::LengthLe => {
            let mut out = command.to_vec();
            out.extend_from_slice(&length);
            out.extend_from_slice(payload);
            out
        }
    }
}

/// 缓冲区中最后一个非零字节之后的长度；`0` 表示完全没被写入。
pub(crate) fn written_len(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .rposition(|byte| *byte != 0)
        .map(|index| index + 1)
        .unwrap_or(0)
}

/// 请求前后是否被厂商代码改写；改写说明 out 参数可能落在了请求缓冲区上。
pub(crate) fn request_changed(before: &[u8], after: &[u8]) -> bool {
    before != after
}

#[cfg(all(target_os = "android", target_arch = "aarch64"))]
mod imp {
    use crate::error::{Result, TfError};
    use crate::rkp::protocol::BUFFER_SIZE;
    use crate::rkp::ta::{BufferInit, CallOptions, CallResult, RawFrame, frame_request};
    use std::ffi::c_void;
    use std::mem::MaybeUninit;

    use libloading::Library;

    const LIBRARY_PATH: &str = "/vendor/lib64/libkeymasterdeviceutils.so";
    /// 观察窗口：足够覆盖 CBOR 头部与帧头。
    const OBSERVE_BYTES: usize = 96;

    type Constructor = unsafe extern "C" fn(*mut c_void, i32);
    type Destructor = unsafe extern "C" fn(*mut c_void);
    type SendCommand = unsafe extern "C" fn(*mut c_void, *mut c_void, u32, *mut c_void, u32) -> i32;

    // 参考实现为该厂商类分配 0x40 字节；只经由构造/析构函数持有，不读取字段。
    // The reference client allocates 0x40 bytes; only the vendor constructor owns them.
    #[repr(C, align(16))]
    struct VendorObject([MaybeUninit<u8>; 0x40]);

    struct Session {
        object: Box<VendorObject>,
        destructor: Destructor,
        send: SendCommand,
        _library: Library,
    }

    fn fail(message: impl Into<String>) -> TfError {
        TfError::new(format!(
            "厂商 KeymasterUtils [vendor KeymasterUtils]: {}",
            message.into()
        ))
    }

    impl Session {
        fn open(level: i32) -> Result<Self> {
            // SAFETY: 仅本适配层解析固定厂商 ABI，且只在 arm64 Android 的隔离子进程中调用。
            // SAFETY: only this adapter resolves the fixed vendor ABI, in an isolated child.
            let library: Library = unsafe {
                libloading::os::unix::Library::open(
                    Some(LIBRARY_PATH),
                    libc::RTLD_NOW | libc::RTLD_LOCAL,
                )
            }
            .map_err(|error| fail(format!("加载 {LIBRARY_PATH} 失败 [load failed]: {error}")))?
            .into();
            let constructor = *unsafe {
                library.get::<Constructor>(
                    b"_ZN14keymasterutils14KeymasterUtilsC1E26keymaster_security_level_t\0",
                )
            }
            .map_err(|_| fail("缺少构造函数符号 [constructor symbol missing]"))?;
            let destructor =
                *unsafe { library.get::<Destructor>(b"_ZN14keymasterutils14KeymasterUtilsD1Ev\0") }
                    .map_err(|_| fail("缺少析构函数符号 [destructor symbol missing]"))?;
            let send = *unsafe {
                library
                    .get::<SendCommand>(b"_ZN14keymasterutils14KeymasterUtils8send_cmdEPvjS1_j\0")
            }
            .map_err(|_| fail("缺少 send_cmd 符号 [send_cmd symbol missing]"))?;
            let mut object = Box::new(VendorObject([MaybeUninit::new(0); 0x40]));
            // SAFETY: 对齐的可写存储地址在 Drop 前保持稳定。
            // SAFETY: stable aligned storage.
            unsafe { constructor(object.as_mut() as *mut VendorObject as *mut c_void, level) };
            Ok(Self {
                object,
                destructor,
                send,
                _library: library,
            })
        }

        fn call(&mut self, request: &[u8], options: CallOptions) -> CallResult {
            let sent = frame_request(request, options.frame);
            let mut request = sent;
            let capacity = options.buffer_len.min(BUFFER_SIZE).max(16);
            let mut response = vec![0u8; capacity];
            match options.buffer {
                BufferInit::Sentinel => {
                    response[..4].copy_from_slice(&i32::MIN.to_le_bytes());
                }
                BufferInit::Zero => {}
                BufferInit::Capacity => {
                    response[..4].copy_from_slice(&(capacity as u32).to_le_bytes());
                }
            }
            let before = request[..request.len().min(OBSERVE_BYTES)].to_vec();
            // SAFETY: 实例存活，两块可写缓冲区在同步调用期间有效且长度匹配。
            // SAFETY: live instance, disjoint buffers valid for the synchronous call.
            let status = unsafe {
                (self.send)(
                    self.object.as_mut() as *mut VendorObject as *mut c_void,
                    request.as_mut_ptr().cast(),
                    request.len() as u32,
                    response.as_mut_ptr().cast(),
                    response.len() as u32,
                )
            };
            let after = request[..request.len().min(OBSERVE_BYTES)].to_vec();
            CallResult {
                status,
                response,
                request_before: before,
                request_after: after,
            }
        }

        fn request(&mut self, request: &[u8]) -> Result<RawFrame> {
            let result = self.call(request, CallOptions::default());
            if result.status != 0 {
                return Err(crate::rkp::explain::status_error(result.status));
            }
            // 哨兵仍在说明厂商完全没有写入；此时直接报 ABI 问题，比让上层猜帧格式准确。
            if result.response.starts_with(&i32::MIN.to_le_bytes()) {
                return Err(TfError::staged(
                    "frame",
                    "abi",
                    "TA 未写入响应缓冲区 [TA never wrote a response]",
                )
                .with_hint("rkp.frame.abi")
                .with_raw(serde_json::json!({
                    "buffer": result.response.len(),
                    "request_changed": super::request_changed(&result.request_before, &result.request_after),
                    "head_hex": crate::rkp::protocol::head_hex(&result.response),
                })));
            }
            Ok(RawFrame {
                bytes: result.response,
                status: result.status,
            })
        }
    }

    impl Drop for Session {
        fn drop(&mut self) {
            // SAFETY: 析构只执行一次，且对象与库在调用期间均存活。
            // SAFETY: the destructor runs once while both the object and library are live.
            unsafe { (self.destructor)(self.object.as_mut() as *mut VendorObject as *mut c_void) };
        }
    }

    pub(super) fn request(bytes: &[u8]) -> Result<RawFrame> {
        Session::open(1)?.request(bytes)
    }

    pub(super) fn probe(bytes: &[u8], options: CallOptions) -> Result<CallResult> {
        Ok(Session::open(options.level)?.call(bytes, options))
    }
}

#[cfg(not(all(target_os = "android", target_arch = "aarch64")))]
mod imp {
    use crate::error::{Result, TfError};
    use crate::rkp::ta::{CallOptions, CallResult, RawFrame};

    pub(super) fn request(_bytes: &[u8]) -> Result<RawFrame> {
        Err(TfError::new(
            "设备端 RKP 只能由 Android arm64 触发 [device RKP requires Android arm64]",
        ))
    }

    pub(super) fn probe(_bytes: &[u8], _options: CallOptions) -> Result<CallResult> {
        Err(TfError::new(
            "设备端 RKP 只能由 Android arm64 触发 [device RKP requires Android arm64]",
        ))
    }
}

pub(crate) fn request(bytes: &[u8]) -> Result<RawFrame> {
    imp::request(bytes)
}

/// 用指定参数执行一次调用，不做状态判定。
pub(crate) fn probe(bytes: &[u8], options: CallOptions) -> Result<CallResult> {
    imp::probe(bytes, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_frame_keeps_bytes() {
        let request = [0x22, 0x31, 0x00, 0x00, 0xa1, 0x01];
        assert_eq!(
            frame_request(&request, RequestFrame::Plain),
            request.to_vec()
        );
    }

    #[test]
    fn length_prefix_is_inserted_after_command() {
        let request = [0x22, 0x31, 0x00, 0x00, 0xa1, 0x01];
        let framed = frame_request(&request, RequestFrame::LengthBe);
        assert_eq!(&framed[..4], &[0x22, 0x31, 0x00, 0x00]);
        assert_eq!(&framed[4..8], &[0, 0, 0, 2]);
        assert_eq!(&framed[8..], &[0xa1, 0x01]);
    }

    #[test]
    fn written_len_ignores_leading_zeros() {
        assert_eq!(written_len(&[0, 0, 0, 0]), 0);
        assert_eq!(written_len(&[0, 0, 7, 0]), 3);
    }

    #[test]
    fn request_change_is_detected() {
        assert!(request_changed(&[1, 2], &[1, 3]));
        assert!(!request_changed(&[1, 2], &[1, 2]));
    }

    #[test]
    fn probe_variants_have_unique_labels() {
        let mut labels = PROBES.iter().map(|(label, _)| *label).collect::<Vec<_>>();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), PROBES.len());
    }
}

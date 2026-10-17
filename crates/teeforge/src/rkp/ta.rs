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

use crate::error::Result;

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

#[cfg(all(target_os = "android", target_arch = "aarch64"))]
mod imp {
    use crate::error::{Result, TfError};
    use crate::rkp::protocol::BUFFER_SIZE;
    use crate::rkp::ta::RawFrame;
    use std::ffi::c_void;
    use std::mem::MaybeUninit;

    use libloading::Library;

    const LIBRARY_PATH: &str = "/vendor/lib64/libkeymasterdeviceutils.so";

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
        fn open() -> Result<Self> {
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
            // SAFETY: 对齐的可写存储地址在 Drop 前保持稳定；security_level 1 为 TEE。
            // SAFETY: stable aligned storage; security_level 1 is TRUSTED_ENVIRONMENT.
            unsafe { constructor(object.as_mut() as *mut VendorObject as *mut c_void, 1) };
            Ok(Self {
                object,
                destructor,
                send,
                _library: library,
            })
        }

        fn request(&mut self, request: &[u8]) -> Result<RawFrame> {
            let mut request = request.to_vec();
            let mut response = vec![0u8; BUFFER_SIZE];
            // 非零哨兵避免未写入的响应被误判为有效。
            // A nonzero sentinel keeps untouched storage from looking valid.
            response[..4].copy_from_slice(&i32::MIN.to_le_bytes());
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
            if status != 0 {
                return Err(crate::rkp::explain::status_error(status));
            }
            Ok(RawFrame {
                bytes: response,
                status,
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
        Session::open()?.request(bytes)
    }
}

#[cfg(not(all(target_os = "android", target_arch = "aarch64")))]
mod imp {
    use crate::error::{Result, TfError};
    use crate::rkp::ta::RawFrame;

    pub(super) fn request(_bytes: &[u8]) -> Result<RawFrame> {
        Err(TfError::new(
            "设备端 RKP 只能由 Android arm64 触发 [device RKP requires Android arm64]",
        ))
    }
}

pub(crate) fn request(bytes: &[u8]) -> Result<RawFrame> {
    imp::request(bytes)
}

use std::{ffi::c_void, ptr};

use windows_sys::Win32::{
    Foundation::GlobalFree,
    System::{
        Console::GetConsoleWindow,
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
        Ole::CF_UNICODETEXT,
    },
};

use super::{ClipboardError, WindowsClipboardApi, copy_windows_with};

struct NativeWindowsClipboard;

impl WindowsClipboardApi for NativeWindowsClipboard {
    type Memory = *mut c_void;

    fn open(&mut self) -> Result<(), ClipboardError> {
        // SAFETY: GetConsoleWindow returns an HWND owned by this process; it is checked
        // before OpenClipboard associates clipboard ownership with it.
        let owner = unsafe { GetConsoleWindow() };
        if owner.is_null() {
            return Err(ClipboardError(
                "cannot open the Windows clipboard because gd has no console window".into(),
            ));
        }
        // SAFETY: owner is a non-null HWND returned for this process's console.
        if unsafe { OpenClipboard(owner) } == 0 {
            return Err(last_error("cannot open the Windows clipboard"));
        }
        Ok(())
    }

    fn empty(&mut self) -> Result<(), ClipboardError> {
        // SAFETY: The clipboard was opened successfully by open and remains open.
        if unsafe { EmptyClipboard() } == 0 {
            return Err(last_error("cannot empty the Windows clipboard"));
        }
        Ok(())
    }

    fn allocate(&mut self, bytes: usize) -> Result<Self::Memory, ClipboardError> {
        // SAFETY: bytes is the exact size of the UTF-16 slice to be copied.
        let memory = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) };
        if memory.is_null() {
            Err(last_error("cannot allocate Windows clipboard memory"))
        } else {
            Ok(memory)
        }
    }

    fn write(&mut self, memory: Self::Memory, wide: &[u16]) -> Result<(), ClipboardError> {
        // SAFETY: memory is a movable allocation sized for wide and remains owned by gd.
        let locked = unsafe { GlobalLock(memory) }.cast::<u16>();
        if locked.is_null() {
            return Err(last_error("cannot lock Windows clipboard memory"));
        }
        // SAFETY: locked points to an allocation of wide.len() u16 values; regions do not
        // overlap and the slice includes its terminating NUL.
        unsafe { ptr::copy_nonoverlapping(wide.as_ptr(), locked, wide.len()) };
        // SAFETY: memory was locked above and is still owned by gd.
        unsafe { GlobalUnlock(memory) };
        Ok(())
    }

    fn set(&mut self, memory: Self::Memory) -> Result<(), ClipboardError> {
        // SAFETY: memory holds NUL-terminated UTF-16 in movable global memory. Ownership
        // transfers to Windows only when SetClipboardData succeeds.
        if unsafe { SetClipboardData(u32::from(CF_UNICODETEXT), memory) }.is_null() {
            Err(last_error("cannot set Unicode Windows clipboard data"))
        } else {
            Ok(())
        }
    }

    fn free(&mut self, memory: Self::Memory) {
        // SAFETY: This is called only before SetClipboardData transfers ownership.
        unsafe { GlobalFree(memory) };
    }

    fn close(&mut self) -> Result<(), ClipboardError> {
        // SAFETY: The clipboard was opened successfully and has not yet been closed.
        if unsafe { CloseClipboard() } == 0 {
            Err(last_error("cannot close the Windows clipboard"))
        } else {
            Ok(())
        }
    }
}

pub(super) fn copy(payload: &[u8]) -> Result<(), ClipboardError> {
    copy_windows_with(payload, &mut NativeWindowsClipboard)
}

fn last_error(context: &str) -> ClipboardError {
    ClipboardError(format!("{context}: {}", std::io::Error::last_os_error()))
}

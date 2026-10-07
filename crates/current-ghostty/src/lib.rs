//! Narrow, benchmark-only bindings compiled against the checked-out Ghostty C headers.
use std::ffi::c_void;
use std::ptr::NonNull;

use libghostty_vt_sys as _; // Builds and links the matching Ghostty VT archive.

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Color { pub r: u8, pub g: u8, pub b: u8, pub valid: u8, pub bold: u8 }

unsafe extern "C" {
    fn bench_new(cols: u16, rows: u16) -> *mut c_void;
    fn bench_free(ptr: *mut c_void);
    fn bench_write(ptr: *mut c_void, bytes: *const u8, len: usize);
    fn bench_resize(ptr: *mut c_void, cols: u16, rows: u16) -> bool;
    fn bench_scroll_top(ptr: *mut c_void);
    fn bench_cursor(ptr: *mut c_void, x: *mut u16, y: *mut u16) -> bool;
    fn bench_snapshot(ptr: *mut c_void, out: *mut u8, cap: usize, fg: *mut Color, bg: *mut Color) -> usize;
}

pub struct Terminal { ptr: NonNull<c_void>, cols: u16, rows: u16 }
impl Terminal {
    pub fn new(cols: u16, rows: u16) -> Self {
        let ptr = NonNull::new(unsafe { bench_new(cols, rows) }).expect("Ghostty terminal allocation failed");
        Self { ptr, cols, rows }
    }
    pub fn feed(&mut self, bytes: &[u8]) { unsafe { bench_write(self.ptr.as_ptr(), bytes.as_ptr(), bytes.len()) } }
    pub fn resize(&mut self, cols: u16, rows: u16) {
        assert!(unsafe { bench_resize(self.ptr.as_ptr(), cols, rows) });
        self.cols = cols; self.rows = rows;
    }
    pub fn cursor(&self) -> (u16, u16) {
        let (mut x, mut y) = (0, 0);
        assert!(unsafe { bench_cursor(self.ptr.as_ptr(), &mut x, &mut y) });
        (x, y)
    }
    pub fn scroll_top(&mut self) { unsafe { bench_scroll_top(self.ptr.as_ptr()) } }
    pub fn snapshot(&self) -> (Vec<String>, Color, Color) {
        let mut buf = vec![0; usize::from(self.cols) * usize::from(self.rows) * 256 + 1024];
        let (mut fg, mut bg) = (Color::default(), Color::default());
        let len = unsafe { bench_snapshot(self.ptr.as_ptr(), buf.as_mut_ptr(), buf.len(), &mut fg, &mut bg) };
        assert!(len <= buf.len(), "Ghostty snapshot failed");
        buf.truncate(len);
        let text = String::from_utf8(buf).expect("Ghostty returned invalid UTF-8");
        let rows = text.split('\n').map(|row| row.trim_end().to_string()).collect();
        (rows, fg, bg)
    }
}
impl Drop for Terminal {
    fn drop(&mut self) { unsafe { bench_free(self.ptr.as_ptr()) } }
}

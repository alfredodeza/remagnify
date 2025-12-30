//! Shared memory buffer management for Wayland.
//!
//! This module handles the creation and management of shared memory buffers
//! used for zero-copy rendering with the Wayland compositor. Buffers are
//! memory-mapped files in XDG_RUNTIME_DIR.

use crate::utils::Vector2D;
use anyhow::{Context, Result};
use cairo::{Context as CairoContext, Format, ImageSurface};
use nix::fcntl::{fcntl, FcntlArg, FdFlag};
use nix::sys::mman::{mmap, munmap, MapFlags, ProtFlags};
use nix::unistd::ftruncate;
use std::num::NonZeroUsize;
use std::os::unix::io::{AsFd, RawFd};
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_shm::WlShm};
use wayland_client::QueueHandle;

/// A memory-mapped shared buffer for Wayland rendering.
///
/// PoolBuffer manages a shared memory region that can be used by both
/// the application and the Wayland compositor for zero-copy rendering.
/// The buffer is backed by a temporary file in XDG_RUNTIME_DIR and is
/// automatically cleaned up when dropped.
pub struct PoolBuffer {
    pub buffer: WlBuffer,
    pub data: *mut u8,
    pub size: usize,
    pub stride: u32,
    pub pixel_size: Vector2D,
    #[allow(dead_code)]
    pub format: u32,
    pub busy: bool,

    // Padded buffer for 24-bit formats
    #[allow(dead_code)]
    pub padded_data: Option<Vec<u8>>,

    // Cairo surface (created on-demand)
    cairo_surface: Option<ImageSurface>,

    // Temp file info
    file_path: String,
}

impl PoolBuffer {
    /// Create a new shared memory buffer.
    ///
    /// Creates a memory-mapped file in XDG_RUNTIME_DIR and sets up a Wayland
    /// buffer that shares this memory with the compositor.
    ///
    /// # Arguments
    ///
    /// * `pixel_size` - Width and height in pixels
    /// * `format` - Pixel format (e.g., ARGB8888)
    /// * `stride` - Bytes per row
    /// * `shm` - Wayland shared memory manager
    /// * `qh` - Wayland event queue handle
    ///
    /// # Returns
    ///
    /// * `Ok(PoolBuffer)` - Successfully created buffer
    /// * `Err` - Failed to create shared memory file or buffer
    ///
    /// # Safety
    ///
    /// Uses unsafe mmap and file descriptor operations, but all are properly
    /// encapsulated and cleaned up via the Drop trait.
    pub fn new<T>(
        pixel_size: Vector2D,
        format: u32,
        stride: u32,
        shm: &WlShm,
        qh: &QueueHandle<T>,
    ) -> Result<Self>
    where
        T: wayland_client::Dispatch<wayland_client::protocol::wl_shm_pool::WlShmPool, ()> + 'static,
        T: wayland_client::Dispatch<WlBuffer, ()> + 'static,
    {
        let size = (stride * pixel_size.y as u32) as usize;

        // Create shared memory file
        let (fd, path) = create_shm_file(size)?;

        // Create Wayland buffer from file descriptor
        use std::os::unix::io::FromRawFd;
        let owned_fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
        let borrowed_fd = owned_fd.as_fd();

        // Memory map
        let data = unsafe {
            mmap(
                None,
                NonZeroUsize::new(size).context("Size cannot be zero")?,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                Some(borrowed_fd),
                0,
            )?
        };

        let pool = shm.create_pool(borrowed_fd, size as i32, qh, ());

        // Convert u32 format to wayland Format enum
        use wayland_client::protocol::wl_shm::Format as WlFormat;
        let wl_format = unsafe { std::mem::transmute::<u32, WlFormat>(format) };

        let buffer = pool.create_buffer(
            0,
            pixel_size.x as i32,
            pixel_size.y as i32,
            stride as i32,
            wl_format,
            qh,
            (),
        );

        pool.destroy();
        // Don't close fd here since owned_fd will do it when dropped
        drop(owned_fd);

        Ok(Self {
            buffer,
            data: data as *mut u8,
            size,
            stride,
            pixel_size,
            format,
            busy: false,
            padded_data: None,
            cairo_surface: None,
            file_path: path,
        })
    }

    /// Get or create a Cairo surface for this buffer.
    ///
    /// Creates a Cairo ImageSurface on first call and caches it for subsequent calls.
    /// The surface provides direct access to the buffer's pixel data.
    ///
    /// # Returns
    ///
    /// * `Ok(&ImageSurface)` - Cairo surface wrapping the buffer data
    /// * `Err` - Cairo surface creation failed
    pub fn get_cairo_surface(&mut self) -> Result<&ImageSurface> {
        if self.cairo_surface.is_none() {
            let surface = unsafe {
                ImageSurface::create_for_data_unsafe(
                    self.data,
                    Format::ARgb32,
                    self.pixel_size.x as i32,
                    self.pixel_size.y as i32,
                    self.stride as i32,
                )?
            };
            self.cairo_surface = Some(surface);
        }

        Ok(self.cairo_surface.as_ref().unwrap())
    }

    /// Create a Cairo context for drawing.
    ///
    /// Convenience method that creates a Cairo context from the buffer's surface.
    ///
    /// # Returns
    ///
    /// * `Ok(CairoContext)` - Context ready for drawing operations
    /// * `Err` - Context creation failed
    pub fn create_cairo_context(&mut self) -> Result<CairoContext> {
        let surface = self.get_cairo_surface()?;
        Ok(CairoContext::new(surface)?)
    }

    /// Mark buffer as busy
    #[allow(dead_code)]
    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }
}

impl Drop for PoolBuffer {
    fn drop(&mut self) {
        unsafe {
            munmap(self.data as *mut _, self.size).ok();
        }
        self.cairo_surface = None;
        std::fs::remove_file(&self.file_path).ok();
    }
}

// SAFETY: The buffer data is only accessed through Wayland callbacks
// which are single-threaded, making it safe to send between threads
unsafe impl Send for PoolBuffer {}

fn create_shm_file(size: usize) -> Result<(RawFd, String)> {
    use nix::libc;

    let xdg_runtime = std::env::var("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR not set")?;

    // Create a template path
    let template = format!("{}/.remagnify_XXXXXX", xdg_runtime);
    let mut path_bytes = template.into_bytes();
    path_bytes.push(0); // Null terminator for C string

    // Use mkstemp to create a unique temporary file
    let fd = unsafe {
        let ret = libc::mkstemp(path_bytes.as_mut_ptr() as *mut libc::c_char);
        if ret < 0 {
            return Err(anyhow::anyhow!("mkstemp failed"));
        }
        ret
    };

    // Remove null terminator and convert back to String
    path_bytes.pop();
    let path = String::from_utf8(path_bytes)?;

    // Set FD_CLOEXEC flag (using raw fd for nix 0.27)
    fcntl(fd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC)).context("Failed to set FD_CLOEXEC")?;

    // Resize the file using raw fd
    use std::os::unix::io::{FromRawFd, IntoRawFd};
    let owned_fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
    ftruncate(&owned_fd, size as i64).context("Failed to truncate file")?;

    // Extract raw fd before owned_fd is dropped
    let raw_fd = owned_fd.into_raw_fd();

    Ok((raw_fd, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::unistd::close;

    #[test]
    fn test_create_shm_file() {
        // This test requires XDG_RUNTIME_DIR to be set
        if std::env::var("XDG_RUNTIME_DIR").is_ok() {
            let result = create_shm_file(1024);
            if let Ok((fd, path)) = result {
                close(fd).ok();
                std::fs::remove_file(path).ok();
            }
        }
    }
}
